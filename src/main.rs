//! majowuji - Personal martial arts training tracker
//!
//! 无极 (wuji) - "limitless", the state of infinite potential

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};

use majowuji::db::{Database, Training};
use majowuji::ml::Analytics;
use majowuji::tui::App;

const DB_PATH: &str = "majowuji.db";

#[derive(Parser)]
#[command(name = "majowuji")]
#[command(author, version, about = "无极 - Personal martial arts training tracker")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open TUI dashboard
    Tui,

    /// Log a training session
    Log {
        /// Exercise name (e.g., "jab", "roundhouse", "forms")
        exercise: String,

        /// Number of sets
        #[arg(short, long, default_value = "1")]
        sets: i32,

        /// Number of reps per set
        #[arg(short, long, default_value = "10")]
        reps: i32,

        /// Optional notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// List training history
    List {
        /// Number of records to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Show training statistics
    Stats {
        /// Filter by exercise name
        exercise: Option<String>,
    },

    /// Start Telegram bot
    Bot {
        /// Telegram bot token (or set TELOXIDE_TOKEN env var)
        #[arg(short, long, env = "TELOXIDE_TOKEN")]
        token: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let db = Database::open(DB_PATH)?;

    match cli.command {
        Some(Commands::Tui) => {
            let mut app = App::new(db)?;
            app.run()?;
        }

        Some(Commands::Log { exercise, sets, reps, notes }) => {
            let training = Training {
                id: None,
                date: Utc::now(),
                exercise: exercise.clone(),
                sets,
                reps,
                duration_secs: None,
                notes,
            };
            let id = db.add_training(&training)?;
            println!("Logged: {} - {}x{} (id: {})", exercise, sets, reps, id);
        }

        Some(Commands::List { limit }) => {
            let trainings = db.get_trainings()?;
            println!("Recent trainings:");
            println!("{:-<60}", "");
            for t in trainings.iter().take(limit) {
                println!(
                    "{} | {:20} | {}x{} | {}",
                    t.date.format("%Y-%m-%d %H:%M"),
                    t.exercise,
                    t.sets,
                    t.reps,
                    t.notes.as_deref().unwrap_or("-")
                );
            }
        }

        Some(Commands::Stats { exercise }) => {
            let trainings = db.get_trainings()?;
            let analytics = Analytics::new(trainings);

            println!("Training Statistics");
            println!("{:-<40}", "");

            if let Some(ex) = exercise {
                let volume = analytics.total_volume(&ex);
                println!("Exercise: {}", ex);
                println!("Total volume: {} reps", volume);

                if let Some((sets, reps)) = analytics.predict_next_load(&ex) {
                    println!("Suggested next: {}x{}", sets, reps);
                }
            } else {
                let freq = analytics.weekly_frequency();
                println!("Weekly frequency: {:.1} sessions/week", freq);
            }
        }

        Some(Commands::Bot { token }) => {
            println!("Starting Telegram bot...");
            println!("База данных: {}", DB_PATH);
            majowuji::bot::run_bot(token, DB_PATH).await?;
        }

        None => {
            // Default: show TUI
            let mut app = App::new(db)?;
            app.run()?;
        }
    }

    Ok(())
}
