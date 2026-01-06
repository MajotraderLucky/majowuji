//! Telegram bot module - Remote training logging with hourly reminders

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, FixedOffset, Utc};
use teloxide::{
    prelude::*,
    types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup},
    utils::command::BotCommands,
    dispatching::dialogue::{InMemStorage, Dialogue},
};
use tokio::sync::Mutex;
use tracing::{info, error};

use crate::db::{Database, Training, User};
use crate::exercises::{get_base_exercises, find_exercise, find_exercise_by_name, EXTRA_EXERCISES};
use crate::ml::{Recommender, ProgressPredictor};
use crate::tips;

/// Bot configuration
pub struct BotConfig {
    pub max_users: usize,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            max_users: std::env::var("MAX_USERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
        }
    }
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type Subscribers = Arc<Mutex<HashSet<ChatId>>>;

/// Reminder interval (1 hour = 3600 seconds)
const REMINDER_INTERVAL_SECS: u64 = 3600;

/// Moscow timezone offset (UTC+3)
const MOSCOW_OFFSET_SECS: i32 = 3 * 3600;

/// Get Moscow timezone for consistent date handling
fn moscow_tz() -> FixedOffset {
    FixedOffset::east_opt(MOSCOW_OFFSET_SECS).unwrap()
}

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
    /// Waiting for message to forward to owner (limit reached)
    WaitingForOwnerMessage,
    /// Waiting for pulse before exercise
    WaitingForPulseBefore {
        exercise_id: String,
        exercise_name: String,
        user_id: i64,
    },
    /// Waiting for reps count (timer running)
    WaitingForReps {
        exercise_id: String,
        exercise_name: String,
        pulse_before: i32,
        start_time: DateTime<Utc>,
        user_id: i64,
    },
    /// Waiting for pulse after exercise
    WaitingForPulseAfter {
        exercise_id: String,
        exercise_name: String,
        pulse_before: i32,
        reps: i32,
        duration_secs: i32,
        user_id: i64,
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
    #[command(description = "Баланс нагрузки по группам мышц")]
    Balance,
    #[command(description = "Включить напоминания раз в час")]
    Remind,
    #[command(description = "Выключить напоминания")]
    Stop,
    #[command(description = "Совет из книги")]
    Tip,
}

/// Create inline keyboard with base exercises
fn make_exercises_keyboard() -> InlineKeyboardMarkup {
    let exercises = get_base_exercises();

    let mut buttons: Vec<Vec<InlineKeyboardButton>> = exercises
        .chunks(2)
        .map(|chunk| {
            chunk.iter().map(|ex| {
                let label = format!("{} {}", ex.category.emoji(), ex.name);
                InlineKeyboardButton::callback(label, format!("ex:{}", ex.id))
            }).collect()
        })
        .collect();

    // Add "From book" button
    buttons.push(vec![
        InlineKeyboardButton::callback("📖 Из книги", "show_extra")
    ]);

    InlineKeyboardMarkup::new(buttons)
}

/// Create inline keyboard with extra exercises from the book
fn make_extra_exercises_keyboard() -> InlineKeyboardMarkup {
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = EXTRA_EXERCISES
        .chunks(2)
        .map(|chunk| {
            chunk.iter().map(|ex| {
                let label = format!("{} {}", ex.category.emoji(), ex.name);
                InlineKeyboardButton::callback(label, format!("ex:{}", ex.id))
            }).collect()
        })
        .collect();

    // Add back button
    buttons.push(vec![
        InlineKeyboardButton::callback("⬅️ Базовые", "show_all")
    ]);

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

/// User access check result
enum AccessResult {
    Allowed(User),
    NewUser(User),
    LimitReached,
}

/// Check user access and register if allowed
fn check_user_access(
    db: &Database,
    chat_id: i64,
    username: Option<&str>,
    first_name: Option<&str>,
    config: &BotConfig,
) -> anyhow::Result<AccessResult> {
    // Check if user already exists
    if let Some(user) = db.get_user_by_chat_id(chat_id)? {
        return Ok(AccessResult::Allowed(user));
    }

    // Check user limit
    let user_count = db.count_users()?;
    if user_count >= config.max_users {
        return Ok(AccessResult::LimitReached);
    }

    // Register new user (first user becomes owner)
    let user = db.get_or_create_user(chat_id, username, first_name)?;

    // Migrate existing trainings to owner if this is the first user
    if user.is_owner {
        let migrated = db.migrate_trainings_to_owner()?;
        if migrated > 0 {
            info!("Migrated {} trainings to owner", migrated);
        }
    }

    Ok(AccessResult::NewUser(user))
}

/// Start the Telegram bot with reminders
pub async fn run_bot(token: String, db_path: &str) -> anyhow::Result<()> {
    let bot = Bot::new(token);
    let db = Arc::new(Mutex::new(Database::open(db_path)?));
    let config = Arc::new(BotConfig::default());
    let subscribers: Subscribers = Arc::new(Mutex::new(HashSet::new()));

    info!("Bot started with max_users={}", config.max_users);

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
        .dependencies(dptree::deps![InMemStorage::<State>::new(), db, config, subscribers])
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
    dialogue: MyDialogue,
    db: Arc<Mutex<Database>>,
    config: Arc<BotConfig>,
    subscribers: Subscribers,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let username = msg.from.as_ref().and_then(|u| u.username.as_deref());
    let first_name = msg.from.as_ref().map(|u| u.first_name.as_str());

    // Check user access
    let user = {
        let db = db.lock().await;
        match check_user_access(&db, chat_id, username, first_name, &config)? {
            AccessResult::Allowed(user) => user,
            AccessResult::NewUser(user) => {
                let welcome = if user.is_owner {
                    "🥋 无极 majowuji\n\n\
                    Ты владелец этого бота!\n\n\
                    /train - выбрать упражнение\n\
                    /today - сегодняшние тренировки\n\
                    /stats - статистика\n\
                    /balance - баланс мышц\n\
                    /remind - напоминания раз в час"
                } else {
                    "🥋 Добро пожаловать в majowuji!\n\n\
                    /train - начать тренировку"
                };
                bot.send_message(msg.chat.id, welcome).await?;
                info!("New user registered: {} (owner={})", chat_id, user.is_owner);
                return Ok(());
            }
            AccessResult::LimitReached => {
                let text = "Бот достиг лимита пользователей (10).\n\n\
                    Напиши сообщение ниже - я передам его владельцу для обсуждения доступа.";
                bot.send_message(msg.chat.id, text).await?;
                dialogue.update(State::WaitingForOwnerMessage).await?;
                return Ok(());
            }
        }
    };

    match cmd {
        Command::Start => {
            let text = "🥋 无极 majowuji\n\n\
                Трекер тренировок боевых искусств\n\n\
                /train - выбрать упражнение\n\
                /today - сегодняшние тренировки\n\
                /stats - статистика\n\
                /balance - баланс мышц\n\
                /remind - напоминания раз в час\n\
                /stop - выключить напоминания";
            bot.send_message(msg.chat.id, text).await?;
        }

        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }

        Command::Train => {
            // Get recommendation based on muscle balance for this user
            let trainings = {
                let db = db.lock().await;
                db.get_trainings_for_user(user.id)?
            };
            let recommender = Recommender::new(trainings);

            if let Some(rec) = recommender.get_recommendation() {
                // Show recommendation with option to choose other
                let text = if rec.is_bonus {
                    // Bonus exercise - show with description
                    let desc = rec.exercise.description.unwrap_or("");
                    format!(
                        "🎁 Бонус! База выполнена!\n\n{} {}\n\n{}\n\n📖 {}\n\nВыбрать или пропустить?",
                        rec.exercise.category.emoji(),
                        rec.exercise.name,
                        rec.reason,
                        desc
                    )
                } else {
                    // Base exercise
                    format!(
                        "🎯 Рекомендую: {} {}\n\n{}\n\nВыбрать рекомендованное или другое?",
                        rec.exercise.category.emoji(),
                        rec.exercise.name,
                        rec.reason
                    )
                };
                let second_button = if rec.is_bonus {
                    InlineKeyboardButton::callback("Пропустить", "skip_bonus")
                } else {
                    InlineKeyboardButton::callback("Выбрать другое", "show_all")
                };
                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback(
                            format!("✓ {}", rec.exercise.name),
                            format!("ex:{}", rec.exercise.id)
                        ),
                    ],
                    vec![second_button],
                ]);
                bot.send_message(msg.chat.id, text)
                    .reply_markup(keyboard)
                    .await?;
            } else {
                // No recommendation, show all exercises
                let keyboard = make_exercises_keyboard();
                bot.send_message(msg.chat.id, "Выбери упражнение:")
                    .reply_markup(keyboard)
                    .await?;
            }
        }

        Command::Today => {
            let db = db.lock().await;
            let trainings = db.get_trainings_for_user(user.id)?;
            let today = Utc::now().with_timezone(&moscow_tz()).date_naive();

            let today_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() == today)
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
            let trainings = db.get_trainings_for_user(user.id)?;

            let total = trainings.len();
            let today = Utc::now().with_timezone(&moscow_tz()).date_naive();
            let week_ago = today - chrono::Duration::days(7);
            let month_ago = today - chrono::Duration::days(30);

            let today_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() == today)
                .collect();

            let week_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() > week_ago)
                .collect();

            let month_trainings: Vec<_> = trainings
                .iter()
                .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() > month_ago)
                .collect();

            let today_time: i32 = today_trainings.iter()
                .filter_map(|t| t.duration_secs)
                .sum();
            let week_time: i32 = week_trainings.iter()
                .filter_map(|t| t.duration_secs)
                .sum();
            let month_time: i32 = month_trainings.iter()
                .filter_map(|t| t.duration_secs)
                .sum();

            let mut text = format!(
                "📈 Статистика\n\n\
                Всего: {} подх.\n\
                Сегодня: {} ({})\n\
                Неделя: {} ({})\n\
                Месяц: {} ({})\n",
                total,
                today_trainings.len(), format_duration(today_time),
                week_trainings.len(), format_duration(week_time),
                month_trainings.len(), format_duration(month_time)
            );

            // Group today's trainings by exercise
            if !today_trainings.is_empty() {
                text.push_str("\n📊 Сегодня:\n");
                // (sets, total_reps, total_time, max_time)
                let mut exercise_stats: std::collections::HashMap<&str, (usize, i32, i32, i32)> = std::collections::HashMap::new();
                for t in &today_trainings {
                    let duration = t.duration_secs.unwrap_or(0);
                    let entry = exercise_stats.entry(&t.exercise).or_insert((0, 0, 0, 0));
                    entry.0 += 1;  // sets count
                    entry.1 += t.reps;  // total reps
                    entry.2 += duration;  // total time
                    entry.3 = entry.3.max(duration);  // max time (record)
                }
                for (exercise, (sets, reps, total_time, max_time)) in exercise_stats {
                    // Check if exercise is timed
                    let is_timed = find_exercise_by_name(exercise)
                        .map(|ex| ex.is_timed)
                        .unwrap_or(false);

                    if is_timed {
                        // For timed exercises: show max time and total
                        text.push_str(&format!(
                            "• {} - {} подх., макс. {}с, всего {}\n",
                            exercise, sets, max_time, format_duration(total_time)
                        ));
                    } else {
                        // For rep-based: show reps and time
                        text.push_str(&format!(
                            "• {} - {} подх., {} повт., {}\n",
                            exercise, sets, reps, format_duration(total_time)
                        ));
                    }
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

        Command::Tip => {
            let tip = tips::get_random_tip();
            let text = format!(
                "📖 Совет из книги\n\"You Are Your Own Gym\"\n\n{}\n\n/tip - ещё совет",
                tips::format_tip(tip)
            );
            bot.send_message(msg.chat.id, text).await?;
        }

        Command::Balance => {
            let trainings = {
                let db = db.lock().await;
                db.get_trainings_for_user(user.id)?
            };
            let recommender = Recommender::new(trainings);
            let report = recommender.get_balance_report();

            bot.send_message(msg.chat.id, format!("🏋️ {}", report)).await?;
        }
    }

    Ok(())
}

async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    db: Arc<Mutex<Database>>,
    config: Arc<BotConfig>,
    _subscribers: Subscribers,
) -> HandlerResult {
    // Get user_id for this callback
    let chat_id = q.message.as_ref().map(|m| m.chat().id.0).unwrap_or(0);
    let username = q.from.username.as_deref();
    let first_name = Some(q.from.first_name.as_str());

    let user = {
        let db = db.lock().await;
        match check_user_access(&db, chat_id, username, first_name, &config)? {
            AccessResult::Allowed(user) | AccessResult::NewUser(user) => user,
            AccessResult::LimitReached => {
                bot.answer_callback_query(q.id).await?;
                return Ok(());
            }
        }
    };

    if let Some(data) = &q.data {
        // Handle "skip bonus" callback
        if data == "skip_bonus" {
            if let Some(msg) = &q.message {
                bot.edit_message_text(
                    msg.chat().id,
                    msg.id(),
                    "👍 База выполнена! Отдыхай.\n\nКогда будешь готов к бонусу - жми /train"
                ).await?;
            }
        }
        // Handle "show all exercises" callback
        else if data == "show_all" {
            let keyboard = make_exercises_keyboard();
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat().id, msg.id(), "Выбери упражнение:")
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        // Handle "show extra exercises" callback
        else if data == "show_extra" {
            let keyboard = make_extra_exercises_keyboard();
            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat().id, msg.id(), "📖 Упражнения из книги:")
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        // Handle exercise selection
        else if let Some(exercise_id) = data.strip_prefix("ex:") {
            if let Some(exercise) = find_exercise(exercise_id) {
                // Set state to waiting for pulse before exercise
                dialogue.update(State::WaitingForPulseBefore {
                    exercise_id: exercise_id.to_string(),
                    exercise_name: exercise.name.to_string(),
                    user_id: user.id,
                }).await?;

                let text = if let Some(desc) = exercise.description {
                    format!(
                        "{} {}\n\n📖 {}\n\nПульс до упражнения?",
                        exercise.category.emoji(),
                        exercise.name,
                        desc
                    )
                } else {
                    format!(
                        "{} {}\n\nПульс до упражнения?",
                        exercise.category.emoji(),
                        exercise.name
                    )
                };

                if let Some(msg) = &q.message {
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
    config: Arc<BotConfig>,
    _subscribers: Subscribers,
) -> HandlerResult {
    let state = dialogue.get().await?.unwrap_or_default();

    match state {
        State::WaitingForOwnerMessage => {
            // Forward message to owner
            if let Some(text) = msg.text() {
                let owner = {
                    let db = db.lock().await;
                    db.get_owner()?
                };

                if let Some(owner) = owner {
                    let from_username = msg.from.as_ref()
                        .and_then(|u| u.username.as_ref())
                        .map(|u| format!("@{}", u))
                        .unwrap_or_else(|| "без username".to_string());
                    let from_name = msg.from.as_ref()
                        .map(|u| u.first_name.as_str())
                        .unwrap_or("Аноним");

                    let forward_text = format!(
                        "📩 Запрос на доступ от {} ({}):\n\n{}",
                        from_username, from_name, text
                    );

                    bot.send_message(ChatId(owner.chat_id), forward_text).await?;
                    bot.send_message(msg.chat.id, "Сообщение отправлено владельцу. Ожидай ответа!").await?;
                    info!("Message forwarded to owner from chat_id={}", msg.chat.id);
                } else {
                    bot.send_message(msg.chat.id, "Ошибка: владелец не найден").await?;
                }

                dialogue.reset().await?;
            }
        }

        State::WaitingForPulseBefore { exercise_id, exercise_name, user_id } => {
            if let Some(text) = msg.text() {
                if let Ok(pulse) = text.trim().parse::<i32>() {
                    if pulse < 30 || pulse > 250 {
                        bot.send_message(msg.chat.id, "Пульс должен быть от 30 до 250").await?;
                        return Ok(());
                    }

                    // Check if exercise is timed (plank) vs rep-based (pushups)
                    let is_timed = find_exercise(&exercise_id)
                        .map(|ex| ex.is_timed)
                        .unwrap_or(false);

                    // Move to waiting for reps, start timer
                    dialogue.update(State::WaitingForReps {
                        exercise_id,
                        exercise_name: exercise_name.clone(),
                        pulse_before: pulse,
                        start_time: Utc::now(),
                        user_id,
                    }).await?;

                    let response = if is_timed {
                        format!(
                            "Пульс: {} уд/мин\n\nВыполняй {}!\n\n⏱ Таймер запущен. Напиши что угодно когда закончишь",
                            pulse, exercise_name
                        )
                    } else {
                        format!(
                            "Пульс: {} уд/мин\n\nВыполняй {}!\n\nСколько повторов?",
                            pulse, exercise_name
                        )
                    };
                    bot.send_message(msg.chat.id, response).await?;
                } else {
                    bot.send_message(msg.chat.id, "Введи пульс (число)").await?;
                }
            }
        }

        State::WaitingForReps { exercise_id, exercise_name, pulse_before, start_time, user_id } => {
            if let Some(text) = msg.text() {
                // Check if exercise is timed
                let is_timed = find_exercise(&exercise_id)
                    .map(|ex| ex.is_timed)
                    .unwrap_or(false);

                if is_timed {
                    // For timed exercises: accept ANY message, calculate duration automatically
                    let now = Utc::now();
                    let elapsed = (now - start_time).num_seconds() as i32;
                    // Subtract 5 seconds for preparation time, minimum 1 second
                    let duration_secs = (elapsed - 5).max(1);
                    let reps = 1;

                    dialogue.update(State::WaitingForPulseAfter {
                        exercise_id,
                        exercise_name: exercise_name.clone(),
                        pulse_before,
                        reps,
                        duration_secs,
                        user_id,
                    }).await?;

                    let response = format!(
                        "⏱ {} - {}с\n\nПульс после упражнения?",
                        exercise_name, duration_secs
                    );
                    bot.send_message(msg.chat.id, response).await?;
                } else {
                    // For rep-based exercises: require a number
                    if let Ok(reps) = text.trim().parse::<i32>() {
                        let now = Utc::now();
                        let duration_secs = (now - start_time).num_seconds() as i32;

                        dialogue.update(State::WaitingForPulseAfter {
                            exercise_id,
                            exercise_name: exercise_name.clone(),
                            pulse_before,
                            reps,
                            duration_secs,
                            user_id,
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
        }

        State::WaitingForPulseAfter { exercise_id, exercise_name, pulse_before, reps, duration_secs, user_id } => {
            if let Some(text) = msg.text() {
                if let Ok(pulse_after) = text.trim().parse::<i32>() {
                    if pulse_after < 30 || pulse_after > 250 {
                        bot.send_message(msg.chat.id, "Пульс должен быть от 30 до 250").await?;
                        return Ok(());
                    }

                    // Check if exercise is timed
                    let is_timed = find_exercise(&exercise_id)
                        .map(|ex| ex.is_timed)
                        .unwrap_or(false);

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
                        user_id: Some(user_id),
                    };

                    // Count today's sets, total time, personal record, and ML prediction
                    let (today_sets, total_time, personal_record, is_new_record, ml_prediction) = {
                        let db = db.lock().await;
                        db.add_training(&training, user_id)?;

                        let trainings = db.get_trainings_for_user(user_id)?;
                        let today = Utc::now().with_timezone(&moscow_tz()).date_naive();

                        // Today's stats
                        let today_exercises: Vec<_> = trainings.iter()
                            .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() == today)
                            .filter(|t| t.exercise == exercise_name)
                            .collect();

                        let sets = today_exercises.len();
                        let time: i32 = today_exercises.iter()
                            .filter_map(|t| t.duration_secs)
                            .sum();

                        // Personal record for this exercise
                        let all_this_exercise: Vec<_> = trainings.iter()
                            .filter(|t| t.exercise == exercise_name)
                            .collect();

                        let total_attempts = all_this_exercise.len();
                        let (record, is_new) = if is_timed {
                            // For timed: max duration
                            let max_duration = all_this_exercise.iter()
                                .filter_map(|t| t.duration_secs)
                                .max()
                                .unwrap_or(0);
                            // New record if beat previous AND not first attempt ever
                            (max_duration, duration_secs >= max_duration && total_attempts > 1)
                        } else {
                            // For rep-based: max reps in single set
                            let max_reps = all_this_exercise.iter()
                                .map(|t| t.reps)
                                .max()
                                .unwrap_or(0);
                            (max_reps, reps >= max_reps && total_attempts > 1)
                        };

                        // ML prediction (only for rep-based exercises with enough data)
                        let prediction = if !is_timed {
                            ProgressPredictor::train(&trainings, &exercise_name)
                                .map(|p| p.format_prediction())
                        } else {
                            None
                        };

                        (sets, time, record, is_new, prediction)
                    };

                    let pulse_diff = pulse_after - pulse_before;
                    let pulse_indicator = if pulse_diff > 30 { "+++" } else if pulse_diff > 15 { "++" } else if pulse_diff > 0 { "+" } else { "-" };

                    let time_str = format_duration(total_time);

                    // Different format for timed vs rep-based exercises
                    let exercise_info = if is_timed {
                        format!("{} - {}с", exercise_name, duration_secs)
                    } else {
                        format!("{} - {} повторов\nВремя: {}с", exercise_name, reps, duration_secs)
                    };

                    // Personal record info
                    let record_info = if is_new_record {
                        if is_timed {
                            format!("🏆 НОВЫЙ РЕКОРД! {}с", personal_record)
                        } else {
                            format!("🏆 НОВЫЙ РЕКОРД! {} повторов", personal_record)
                        }
                    } else {
                        if is_timed {
                            format!("Рекорд: {}с", personal_record)
                        } else {
                            format!("Рекорд: {} повторов", personal_record)
                        }
                    };

                    // Build response with optional ML prediction
                    let ml_section = ml_prediction
                        .map(|p| format!("\n\n{}", p))
                        .unwrap_or_default();

                    let response = format!(
                        "Записано!\n\n\
                        {}\n\
                        Пульс: {} -> {} ({}{}) уд/мин\n\n\
                        {}\n\
                        Сегодня: {} подх., {}{}\n\n\
                        📋 Команды:\n\
                        /train - ещё упражнение\n\
                        /stats - статистика\n\
                        /balance - баланс мышц\n\
                        /tip - совет",
                        exercise_info,
                        pulse_before, pulse_after, pulse_indicator, pulse_diff,
                        record_info,
                        today_sets, time_str,
                        ml_section
                    );

                    bot.send_message(msg.chat.id, response).await?;
                    dialogue.reset().await?;
                } else {
                    bot.send_message(msg.chat.id, "Введи пульс (число)").await?;
                }
            }
        }

        State::Start => {
            // Check if user exists, if not - might need registration check
            let chat_id = msg.chat.id.0;
            let username = msg.from.as_ref().and_then(|u| u.username.as_deref());
            let first_name = msg.from.as_ref().map(|u| u.first_name.as_str());

            let access = {
                let db = db.lock().await;
                check_user_access(&db, chat_id, username, first_name, &config)?
            };

            match access {
                AccessResult::LimitReached => {
                    let text = "Бот достиг лимита пользователей (10).\n\n\
                        Напиши сообщение ниже - я передам его владельцу для обсуждения доступа.";
                    bot.send_message(msg.chat.id, text).await?;
                    dialogue.update(State::WaitingForOwnerMessage).await?;
                }
                _ => {
                    // User is registered, suggest /train
                    bot.send_message(msg.chat.id, "Жми /train чтобы начать тренировку")
                        .await?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moscow_tz_offset() {
        let tz = moscow_tz();
        // Moscow is UTC+3 = 3 * 3600 = 10800 seconds
        assert_eq!(tz.local_minus_utc(), 10800);
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(5), "5с");
        assert_eq!(format_duration(30), "30с");
        assert_eq!(format_duration(59), "59с");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60), "1м 0с");
        assert_eq!(format_duration(90), "1м 30с");
        assert_eq!(format_duration(125), "2м 5с");
        assert_eq!(format_duration(3599), "59м 59с");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600), "1ч 0м");
        assert_eq!(format_duration(3660), "1ч 1м");
        assert_eq!(format_duration(7200), "2ч 0м");
        assert_eq!(format_duration(7260), "2ч 1м");
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "0с");
    }

    #[test]
    fn test_reminder_interval_constant() {
        // 1 hour = 3600 seconds
        assert_eq!(REMINDER_INTERVAL_SECS, 3600);
    }

    #[test]
    fn test_moscow_offset_constant() {
        // UTC+3 = 3 * 3600 = 10800
        assert_eq!(MOSCOW_OFFSET_SECS, 10800);
    }

    #[test]
    fn test_bot_config_default() {
        // Note: this test may fail if MAX_USERS env var is set
        // Default max_users should be 10
        let config = BotConfig::default();
        assert_eq!(config.max_users, 10);
    }
}
