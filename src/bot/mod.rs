//! Telegram bot module - Remote training logging

use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Show help")]
    Help,
    #[command(description = "Log training: /log <exercise> <sets>x<reps>")]
    Log(String),
    #[command(description = "Show today's trainings")]
    Today,
    #[command(description = "Show statistics")]
    Stats,
}

/// Start the Telegram bot
pub async fn run_bot(token: String) -> anyhow::Result<()> {
    let bot = Bot::new(token);

    Command::repl(bot, answer).await;
    Ok(())
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Start => {
            bot.send_message(msg.chat.id, "无极 majowuji - Training Tracker\n\nUse /help to see commands")
                .await?;
        }
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Log(input) => {
            // Parse format: "exercise sets reps" or "exercise setsxreps"
            let parts: Vec<&str> = input.split_whitespace().collect();
            let response = if parts.len() >= 2 {
                let exercise = parts[0];
                let (sets, reps) = if parts.len() == 2 && parts[1].contains('x') {
                    // Format: "jab 3x10"
                    let sr: Vec<&str> = parts[1].split('x').collect();
                    (sr[0].parse().unwrap_or(1), sr[1].parse().unwrap_or(10))
                } else if parts.len() >= 3 {
                    // Format: "jab 3 10"
                    (parts[1].parse().unwrap_or(1), parts[2].parse().unwrap_or(10))
                } else {
                    (1, 10)
                };
                // TODO: Save to database
                format!("Logged: {} - {}x{}\n(Database integration pending)", exercise, sets, reps)
            } else {
                "Usage: /log <exercise> <sets>x<reps>\nExample: /log jab 3x10".to_string()
            };
            bot.send_message(msg.chat.id, response).await?;
        }
        Command::Today => {
            // TODO: Fetch from database
            bot.send_message(msg.chat.id, "Today's trainings:\n(Coming soon)")
                .await?;
        }
        Command::Stats => {
            // TODO: Calculate statistics
            bot.send_message(msg.chat.id, "Statistics:\n(Coming soon)")
                .await?;
        }
    }
    Ok(())
}
