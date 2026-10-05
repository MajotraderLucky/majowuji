//! majowuji - Personal martial arts training tracker
//!
//! 无极 (wuji) - "limitless", the state of infinite potential

use std::path::Path;

use anyhow::{Result, bail};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde_json::json;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

use majowuji::db::{Database, Training};
use majowuji::intervals;
use majowuji::ml::Analytics;
use majowuji::tui::App;

#[derive(Parser)]
#[command(name = "majowuji")]
#[command(author, version, about = "无极 - Personal martial arts training tracker")]
struct Cli {
    /// Path to SQLite database
    #[arg(long, global = true, env = "MAJOWUJI_DB", default_value = "majowuji.db")]
    db: String,

    /// Machine-readable JSON output on stdout (list, stats, log)
    #[arg(long, global = true)]
    json: bool,

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
        #[arg(short, long, default_value = "1", value_parser = clap::value_parser!(i32).range(1..))]
        sets: i32,

        /// Number of reps per set
        #[arg(short, long, default_value = "10", value_parser = clap::value_parser!(i32).range(1..))]
        reps: i32,

        /// Duration in seconds, for timed exercises (plank, stance, ...)
        #[arg(long, value_parser = clap::value_parser!(i32).range(1..))]
        duration: Option<i32>,

        /// Heart rate before the set, bpm
        #[arg(long, value_parser = clap::value_parser!(i32).range(30..=220))]
        pulse_before: Option<i32>,

        /// Heart rate after the set, bpm
        #[arg(long, value_parser = clap::value_parser!(i32).range(30..=220))]
        pulse_after: Option<i32>,

        /// Optional notes
        #[arg(short, long)]
        notes: Option<String>,

        /// Create and initialize the database if the file does not exist
        #[arg(long)]
        create: bool,
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

    /// Diary of imported watch workouts (from `intervals sync`)
    Activities {
        /// Number of activities to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Create or migrate the database schema (no training records are added)
    Migrate,

    /// Watch data from Intervals.icu (API key in INTERVALS_API_KEY)
    Intervals {
        #[command(subcommand)]
        action: IntervalsAction,
    },

    /// Start Telegram bot
    Bot {
        /// Telegram bot token (or set TELOXIDE_TOKEN env var)
        #[arg(short, long, env = "TELOXIDE_TOKEN")]
        token: String,
    },
}

#[derive(Subcommand)]
enum IntervalsAction {
    /// Import watch workouts and fill heart rate of logged sets
    Sync {
        /// Days back from today to import
        #[arg(long, default_value = "7", value_parser = clap::value_parser!(i64).range(1..=365))]
        days: i64,

        /// Intervals.icu athlete id (e.g. i123456)
        #[arg(long, env = "MAJOWUJI_INTERVALS_ATHLETE")]
        athlete: String,

        /// Create and initialize the database if the file does not exist
        #[arg(long)]
        create: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // stdout is the product (JSON for agents), diagnostics go to stderr.
    // RUST_LOG is parsed as Targets (as fmt::init() does without the env-filter feature);
    // unset or unparsable RUST_LOG falls back to INFO.
    let targets = std::env::var("RUST_LOG")
        .ok()
        .and_then(|v| v.parse::<Targets>().ok())
        .unwrap_or_else(|| Targets::new().with_default(tracing::Level::INFO));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(targets)
        .init();

    let cli = Cli::parse();

    // Only a real file path: in-memory databases would make `log` a false success
    if cli.db.is_empty() || cli.db == ":memory:" || cli.db.starts_with("file:") {
        bail!("--db must be a file path, got {:?}", cli.db);
    }
    let is_tui = matches!(cli.command, Some(Commands::Tui) | None);
    if cli.json && (is_tui || matches!(cli.command, Some(Commands::Bot { .. }) | Some(Commands::Migrate))) {
        bail!("--json is supported only for list, stats, activities, log and intervals sync");
    }

    // migrate updates an existing database only: a typo in the path must not yield a new empty one
    if matches!(cli.command, Some(Commands::Migrate))
        && std::fs::metadata(&cli.db).map(|m| m.len() == 0).unwrap_or(true)
    {
        bail!("database not found or empty: {}", cli.db);
    }

    // Read commands must neither create nor migrate the database
    let writes_db = matches!(
        cli.command,
        Some(Commands::Log { .. })
            | Some(Commands::Bot { .. })
            | Some(Commands::Migrate)
            | Some(Commands::Intervals { .. })
    );
    let db = if writes_db {
        // No silent creation from a wrong cwd: an agent shell without MAJOWUJI_DB
        // must fail loudly, not seed an empty database somewhere else
        let may_create = match &cli.command {
            Some(Commands::Log { create, .. }) => *create,
            Some(Commands::Intervals { action: IntervalsAction::Sync { create, .. } }) => *create,
            // `migrate` is the explicit command for updating an existing database
            Some(Commands::Migrate) => true,
            _ => false,
        };
        if !may_create && !Path::new(&cli.db).exists() {
            bail!(
                "database not found: {} (pass --db / set MAJOWUJI_DB, or add --create to initialize)",
                cli.db
            );
        }
        if matches!(cli.command, Some(Commands::Migrate)) {
            // migrate never creates the file: existing-only open + explicit init
            Database::open_for_migrate(&cli.db)?
        } else if may_create {
            // `--create` initializes only a missing or empty file; an existing
            // database — current, outdated or foreign — is never migrated here
            Database::open_create(&cli.db)?
        } else {
            // an existing file must already be a majowuji database: an empty or
            // foreign SQLite file is not silently initialized (contract: --create)
            Database::open_existing(&cli.db)?
        }
    } else {
        if !Path::new(&cli.db).exists() {
            bail!("database not found: {}", cli.db);
        }
        Database::open_readonly(&cli.db)?
    };

    match cli.command {
        Some(Commands::Tui) | None => {
            let mut app = App::new(db)?;
            app.run()?;
        }

        Some(Commands::Log { exercise, sets, reps, duration, pulse_before, pulse_after, notes, .. }) => {
            let exercise = exercise.trim().to_string();
            if exercise.is_empty() {
                bail!("exercise name must not be empty");
            }
            let training = Training {
                id: None,
                date: Utc::now(),
                exercise: exercise.clone(),
                sets,
                reps,
                duration_secs: duration,
                pulse_before,
                pulse_after,
                notes,
                user_id: None,
            };
            let id = db.add_training_cli(&training)?;
            if cli.json {
                let logged = Training { id: Some(id), ..training };
                println!("{}", serde_json::to_string_pretty(&logged)?);
            } else {
                println!("Logged: {} - {}x{} (id: {})", exercise, sets, reps, id);
            }
        }

        Some(Commands::List { limit }) => {
            let trainings = db.get_trainings()?;
            if cli.json {
                let recent: Vec<_> = trainings.iter().take(limit).collect();
                println!("{}", serde_json::to_string_pretty(&recent)?);
                return Ok(());
            }
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

        Some(Commands::Activities { limit }) => {
            let activities = db.get_watch_activities(limit)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&activities)?);
                return Ok(());
            }
            println!("Watch activities:");
            println!("{:-<60}", "");
            for a in activities {
                println!(
                    "{} | {} | {:16} | {:>5} s | pulse {}-{} | {:>4} kcal | {}",
                    a.id,
                    a.start.format("%Y-%m-%d %H:%M"),
                    a.activity_type.as_deref().or(a.name.as_deref()).unwrap_or("-"),
                    a.elapsed_secs,
                    a.avg_hr.map_or("-".to_string(), |v| v.to_string()),
                    a.max_hr.map_or("-".to_string(), |v| v.to_string()),
                    a.calories.map_or("-".to_string(), |v| v.to_string()),
                    a.source.as_deref().unwrap_or("-"),
                );
            }
        }

        Some(Commands::Stats { exercise }) => {
            let trainings = db.get_trainings()?;
            let total = trainings.len();
            let analytics = Analytics::new(trainings);

            if cli.json {
                let stats = match &exercise {
                    Some(ex) => json!({
                        "exercise": ex,
                        "total_volume": analytics.total_volume(ex),
                        "suggested_next": analytics
                            .predict_next_load(ex)
                            .map(|(sets, reps)| json!({ "sets": sets, "reps": reps })),
                    }),
                    None => json!({
                        "total_trainings": total,
                        "weekly_frequency": analytics.weekly_frequency(),
                    }),
                };
                println!("{}", serde_json::to_string_pretty(&stats)?);
                return Ok(());
            }

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

        Some(Commands::Intervals { action: IntervalsAction::Sync { days, athlete, .. } }) => {
            // The key comes from the environment only: never from argv (visible in ps and logs)
            let api_key = std::env::var("INTERVALS_API_KEY").unwrap_or_default();
            let client = intervals::Client::new(api_key, athlete)?;
            // Intervals.icu filters by the athlete's local date, which may be a day ahead
            // or behind UTC: widen the window by one day on both sides
            let today = Utc::now().date_naive();
            let one_day = chrono::Duration::days(1);
            let sessions = client
                .sessions(today - chrono::Duration::days(days) - one_day, today + one_day)
                .await?;
            for s in &sessions {
                db.upsert_watch_session(s)?;
            }
            // Only CLI-created records (user_id NULL): bot rows belong to their
            // users — a training of another user inside the watch window must
            // never receive the owner's watch pulse (AGENTS.md "Known limitation")
            let cli_trainings: Vec<_> = db
                .get_trainings()?
                .into_iter()
                .filter(|t| t.user_id.is_none())
                .collect();
            let links = intervals::link_pulses(&cli_trainings, &sessions);
            let mut filled = Vec::new();
            for l in links {
                // the report shows what is actually stored, kept real readings included
                if let Some((before, after)) =
                    db.fill_training_pulse(l.training_id, l.pulse_before, l.pulse_after)?
                {
                    filled.push(intervals::PulseLink {
                        training_id: l.training_id,
                        session_id: l.session_id.clone(),
                        pulse_before: before,
                        pulse_after: after,
                    });
                }
            }
            if cli.json {
                let report = json!({ "sessions": sessions.len(), "filled": filled });
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Watch sessions imported: {}", sessions.len());
                let fmt = |v: Option<i32>| v.map_or_else(|| "-".to_string(), |b| b.to_string());
                for l in &filled {
                    println!(
                        "training {} <- {}: pulse {} -> {}",
                        l.training_id,
                        l.session_id,
                        fmt(l.pulse_before),
                        fmt(l.pulse_after)
                    );
                }
            }
        }

        Some(Commands::Migrate) => {
            // open_for_migrate has run init_schema; ALTER errors propagate,
            // this verifies the file really ended up current (e.g. no virtual
            // table tricks blocking an ALTER)
            if !db.schema_is_current() {
                bail!("migration did not complete (read-only file?): {}", cli.db);
            }
            println!("Database schema is up to date: {}", cli.db);
        }

        Some(Commands::Bot { token }) => {
            println!("Starting Telegram bot...");
            println!("База данных: {}", cli.db);
            majowuji::bot::run_bot(token, &cli.db).await?;
        }
    }

    Ok(())
}
