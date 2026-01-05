//! Telegram bot module - Remote training logging with hourly reminders

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Local, Utc};
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

/// Format duration in seconds to human-readable string
fn format_duration(secs: i32) -> String {
    if secs < 60 {
        format!("{}с", secs)
    } else if secs < 3600 {
        format!("{}м {}с", secs / 60, secs % 60)
    } else {
        format!("{}ч {}м", secs / 3600, (secs % 3600) / 60)
    }
}

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    /// Waiting for pulse before exercise
    WaitingForPulseBefore {
        exercise_id: String,
        exercise_name: String,
    },
    /// Waiting for reps count (timer running)
    WaitingForReps {
        exercise_id: String,
        exercise_name: String,
        pulse_before: i32,
        start_time: DateTime<Utc>,
    },
    /// Waiting for pulse after exercise
    WaitingForPulseAfter {
        exercise_id: String,
        exercise_name: String,
        pulse_before: i32,
        reps: i32,
        duration_secs: i32,
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
            let today_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&Local).date_naive() == today)
                .collect();

            let today_time: i32 = today_trainings.iter()
                .filter_map(|t| t.duration_secs)
                .sum();

            let mut text = format!(
                "📈 Статистика\n\n\
                Всего подходов: {}\n\
                Сегодня: {} ({})\n",
                total, today_trainings.len(), format_duration(today_time)
            );

            // Group today's trainings by exercise
            if !today_trainings.is_empty() {
                text.push_str("\n📊 Сегодня:\n");
                let mut exercise_stats: std::collections::HashMap<&str, (usize, i32, i32)> = std::collections::HashMap::new();
                for t in &today_trainings {
                    let entry = exercise_stats.entry(&t.exercise).or_insert((0, 0, 0));
                    entry.0 += 1;  // sets count
                    entry.1 += t.reps;  // total reps
                    entry.2 += t.duration_secs.unwrap_or(0);  // total time
                }
                for (exercise, (sets, reps, time)) in exercise_stats {
                    text.push_str(&format!("• {} - {} подх., {} повт., {}\n", exercise, sets, reps, format_duration(time)));
                }
            }

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
                // Set state to waiting for pulse before exercise
                dialogue.update(State::WaitingForPulseBefore {
                    exercise_id: exercise_id.to_string(),
                    exercise_name: exercise.name.to_string(),
                }).await?;

                let text = format!(
                    "{} {}\n\nПульс до упражнения?",
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
        State::WaitingForPulseBefore { exercise_id, exercise_name } => {
            if let Some(text) = msg.text() {
                if let Ok(pulse) = text.trim().parse::<i32>() {
                    if pulse < 30 || pulse > 250 {
                        bot.send_message(msg.chat.id, "Пульс должен быть от 30 до 250").await?;
                        return Ok(());
                    }

                    // Move to waiting for reps, start timer
                    dialogue.update(State::WaitingForReps {
                        exercise_id,
                        exercise_name: exercise_name.clone(),
                        pulse_before: pulse,
                        start_time: Utc::now(),
                    }).await?;

                    let response = format!(
                        "Пульс: {} уд/мин\n\nВыполняй {}!\n\nСколько повторов?",
                        pulse, exercise_name
                    );
                    bot.send_message(msg.chat.id, response).await?;
                } else {
                    bot.send_message(msg.chat.id, "Введи пульс (число)").await?;
                }
            }
        }

        State::WaitingForReps { exercise_id, exercise_name, pulse_before, start_time } => {
            if let Some(text) = msg.text() {
                if let Ok(reps) = text.trim().parse::<i32>() {
                    // Calculate duration
                    let now = Utc::now();
                    let duration_secs = (now - start_time).num_seconds() as i32;

                    // Move to waiting for pulse after
                    dialogue.update(State::WaitingForPulseAfter {
                        exercise_id,
                        exercise_name: exercise_name.clone(),
                        pulse_before,
                        reps,
                        duration_secs,
                    }).await?;

                    let response = format!(
                        "{} - {} повторов за {}с\n\nПульс после упражнения?",
                        exercise_name, reps, duration_secs
                    );
                    bot.send_message(msg.chat.id, response).await?;
                } else {
                    bot.send_message(msg.chat.id, "Введи число повторов").await?;
                }
            }
        }

        State::WaitingForPulseAfter { exercise_id: _, exercise_name, pulse_before, reps, duration_secs } => {
            if let Some(text) = msg.text() {
                if let Ok(pulse_after) = text.trim().parse::<i32>() {
                    if pulse_after < 30 || pulse_after > 250 {
                        bot.send_message(msg.chat.id, "Пульс должен быть от 30 до 250").await?;
                        return Ok(());
                    }

                    // Save to database
                    let training = Training {
                        id: None,
                        date: Utc::now(),
                        exercise: exercise_name.clone(),
                        sets: 1,
                        reps,
                        duration_secs: Some(duration_secs),
                        pulse_before: Some(pulse_before),
                        pulse_after: Some(pulse_after),
                        notes: None,
                    };

                    // Count today's sets and total time for this exercise
                    let (today_sets, total_time) = {
                        let db = db.lock().await;
                        db.add_training(&training)?;

                        let trainings = db.get_trainings()?;
                        let today = Local::now().date_naive();
                        let today_exercises: Vec<_> = trainings.iter()
                            .filter(|t| t.date.with_timezone(&Local).date_naive() == today)
                            .filter(|t| t.exercise == exercise_name)
                            .collect();

                        let sets = today_exercises.len();
                        let time: i32 = today_exercises.iter()
                            .filter_map(|t| t.duration_secs)
                            .sum();
                        (sets, time)
                    };

                    let pulse_diff = pulse_after - pulse_before;
                    let pulse_indicator = if pulse_diff > 30 { "+++" } else if pulse_diff > 15 { "++" } else if pulse_diff > 0 { "+" } else { "-" };

                    let time_str = format_duration(total_time);
                    let response = format!(
                        "Записано!\n\n\
                        {} - {} повторов\n\
                        Время: {}с\n\
                        Пульс: {} -> {} ({}{}) уд/мин\n\n\
                        Сегодня: {} подх., {}\n\n\
                        /train - ещё",
                        exercise_name, reps, duration_secs,
                        pulse_before, pulse_after, pulse_indicator, pulse_diff,
                        today_sets, time_str
                    );

                    bot.send_message(msg.chat.id, response).await?;
                    dialogue.reset().await?;
                } else {
                    bot.send_message(msg.chat.id, "Введи пульс (число)").await?;
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
