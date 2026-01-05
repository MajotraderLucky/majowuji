//! Telegram bot module - Remote training logging with hourly reminders

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use chrono::{Local, Utc};
use teloxide::{
    prelude::*,
    types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup},
    utils::command::BotCommands,
    dispatching::dialogue::{InMemStorage, Dialogue},
};
use tokio::sync::Mutex;
use tracing::{info, error};

use crate::db::{Database, Training};
use crate::exercises::{get_base_exercises, find_exercise};

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type Subscribers = Arc<Mutex<HashSet<ChatId>>>;

/// Reminder interval (1 hour = 3600 seconds)
const REMINDER_INTERVAL_SECS: u64 = 3600;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    WaitingForReps {
        exercise_id: String,
        exercise_name: String,
    },
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Команды бота:")]
pub enum Command {
    #[command(description = "Начать работу")]
    Start,
    #[command(description = "Показать помощь")]
    Help,
    #[command(description = "Выбрать упражнение")]
    Train,
    #[command(description = "Сегодняшние тренировки")]
    Today,
    #[command(description = "Статистика")]
    Stats,
    #[command(description = "Включить напоминания раз в час")]
    Remind,
    #[command(description = "Выключить напоминания")]
    Stop,
}

/// Create inline keyboard with base exercises
fn make_exercises_keyboard() -> InlineKeyboardMarkup {
    let exercises = get_base_exercises();

    let buttons: Vec<Vec<InlineKeyboardButton>> = exercises
        .chunks(2)
        .map(|chunk| {
            chunk.iter().map(|ex| {
                let label = format!("{} {}", ex.category.emoji(), ex.name);
                InlineKeyboardButton::callback(label, format!("ex:{}", ex.id))
            }).collect()
        })
        .collect();

    InlineKeyboardMarkup::new(buttons)
}

/// Background task that sends reminders every hour
async fn reminder_task(bot: Bot, subscribers: Subscribers) {
    info!("Reminder task started (interval: {} seconds)", REMINDER_INTERVAL_SECS);

    loop {
        tokio::time::sleep(Duration::from_secs(REMINDER_INTERVAL_SECS)).await;

        let subs = subscribers.lock().await;
        if subs.is_empty() {
            continue;
        }

        info!("Sending reminders to {} subscribers", subs.len());
        let keyboard = make_exercises_keyboard();

        for chat_id in subs.iter() {
            let result = bot
                .send_message(*chat_id, "⏰ Время размяться!\n\nВыбери упражнение:")
                .reply_markup(keyboard.clone())
                .await;

            if let Err(e) = result {
                error!("Failed to send reminder to {}: {}", chat_id, e);
            }
        }
    }
}

/// Start the Telegram bot with reminders
pub async fn run_bot(token: String, db_path: &str) -> anyhow::Result<()> {
    let bot = Bot::new(token);
    let db = Arc::new(Mutex::new(Database::open(db_path)?));
    let subscribers: Subscribers = Arc::new(Mutex::new(HashSet::new()));

    // Start reminder background task
    let reminder_bot = bot.clone();
    let reminder_subs = subscribers.clone();
    tokio::spawn(async move {
        reminder_task(reminder_bot, reminder_subs).await;
    });

    let handler = dptree::entry()
        .enter_dialogue::<Update, InMemStorage<State>, State>()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(
            Update::filter_message()
                .endpoint(handle_message),
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(handle_callback),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new(), db, subscribers])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    _dialogue: MyDialogue,
    db: Arc<Mutex<Database>>,
    subscribers: Subscribers,
) -> HandlerResult {
    match cmd {
        Command::Start => {
            let text = "🥋 无极 majowuji\n\n\
                Трекер тренировок боевых искусств\n\n\
                /train - выбрать упражнение\n\
                /today - сегодняшние тренировки\n\
                /stats - статистика\n\
                /remind - напоминания раз в час\n\
                /stop - выключить напоминания";
            bot.send_message(msg.chat.id, text).await?;
        }

        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }

        Command::Train => {
            let keyboard = make_exercises_keyboard();
            bot.send_message(msg.chat.id, "Выбери упражнение:")
                .reply_markup(keyboard)
                .await?;
        }

        Command::Today => {
            let db = db.lock().await;
            let trainings = db.get_trainings()?;
            let today = Local::now().date_naive();

            let today_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&Local).date_naive() == today)
                .collect();

            if today_trainings.is_empty() {
                bot.send_message(msg.chat.id, "Сегодня ещё не было тренировок. Жми /train!")
                    .await?;
            } else {
                let mut text = String::from("📊 Сегодня:\n\n");
                for t in today_trainings {
                    text.push_str(&format!(
                        "• {} - {}x{}\n",
                        t.exercise, t.sets, t.reps
                    ));
                }
                bot.send_message(msg.chat.id, text).await?;
            }
        }

        Command::Stats => {
            let db = db.lock().await;
            let trainings = db.get_trainings()?;

            let total = trainings.len();
            let today = Local::now().date_naive();
            let today_count = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&Local).date_naive() == today)
                .count();

            let text = format!(
                "📈 Статистика\n\n\
                Всего тренировок: {}\n\
                Сегодня: {}",
                total, today_count
            );

            bot.send_message(msg.chat.id, text).await?;
        }

        Command::Remind => {
            let mut subs = subscribers.lock().await;
            subs.insert(msg.chat.id);
            let count = subs.len();

            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Напоминания включены!\n\n\
                    Буду напоминать раз в час.\n\
                    /stop - выключить\n\n\
                    Активных подписчиков: {}",
                    count
                )
            ).await?;

            info!("User {} subscribed to reminders", msg.chat.id);
        }

        Command::Stop => {
            let mut subs = subscribers.lock().await;
            let was_subscribed = subs.remove(&msg.chat.id);

            if was_subscribed {
                bot.send_message(msg.chat.id, "🔕 Напоминания выключены.\n\n/remind - включить снова")
                    .await?;
                info!("User {} unsubscribed from reminders", msg.chat.id);
            } else {
                bot.send_message(msg.chat.id, "Напоминания и так выключены.\n\n/remind - включить")
                    .await?;
            }
        }
    }

    Ok(())
}

async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    _db: Arc<Mutex<Database>>,
    _subscribers: Subscribers,
) -> HandlerResult {
    if let Some(data) = &q.data {
        if let Some(exercise_id) = data.strip_prefix("ex:") {
            if let Some(exercise) = find_exercise(exercise_id) {
                // Set state to waiting for reps
                dialogue.update(State::WaitingForReps {
                    exercise_id: exercise_id.to_string(),
                    exercise_name: exercise.name.to_string(),
                }).await?;

                let text = format!(
                    "{} {}\n\nВведи результат в формате:\nсеты x повторы\n\nПример: 3x20 или 3 20",
                    exercise.category.emoji(),
                    exercise.name
                );

                if let Some(msg) = q.message {
                    bot.edit_message_text(msg.chat().id, msg.id(), text)
                        .await?;
                }
            }
        }
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Arc<Mutex<Database>>,
    _subscribers: Subscribers,
) -> HandlerResult {
    let state = dialogue.get().await?.unwrap_or_default();

    match state {
        State::WaitingForReps { exercise_id: _, exercise_name } => {
            if let Some(text) = msg.text() {
                // Parse "3x20" or "3 20"
                let parts: Vec<&str> = text.split(|c| c == 'x' || c == 'х' || c == ' ')
                    .filter(|s| !s.is_empty())
                    .collect();

                if parts.len() >= 2 {
                    let sets: i32 = parts[0].parse().unwrap_or(1);
                    let reps: i32 = parts[1].parse().unwrap_or(10);

                    // Save to database
                    let training = Training {
                        id: None,
                        date: Utc::now(),
                        exercise: exercise_name.clone(),
                        sets,
                        reps,
                        notes: None,
                    };

                    {
                        let db = db.lock().await;
                        db.add_training(&training)?;
                    }

                    let response = format!(
                        "✅ Записано!\n\n{} - {}x{}\n\n/train - ещё упражнение\n/today - результаты дня",
                        exercise_name, sets, reps
                    );

                    bot.send_message(msg.chat.id, response).await?;

                    dialogue.reset().await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Не понял. Введи в формате: 3x20 или 3 20"
                    ).await?;
                }
            }
        }
        State::Start => {
            // Unknown message, suggest /train
            bot.send_message(msg.chat.id, "Жми /train чтобы начать тренировку")
                .await?;
        }
    }

    Ok(())
}
