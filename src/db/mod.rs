//! Database module - SQLite storage for training data

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

/// Training session record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Training {
    pub id: Option<i64>,
    pub date: DateTime<Utc>,
    pub exercise: String,
    pub sets: i32,
    pub reps: i32,
    pub duration_secs: Option<i32>,  // Time spent on exercise
    pub pulse_before: Option<i32>,   // Heart rate before exercise
    pub pulse_after: Option<i32>,    // Heart rate after exercise
    pub notes: Option<String>,
}

/// Database wrapper
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create database
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS trainings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                exercise TEXT NOT NULL,
                sets INTEGER NOT NULL,
                reps INTEGER NOT NULL,
                duration_secs INTEGER,
                pulse_before INTEGER,
                pulse_after INTEGER,
                notes TEXT
            )",
            [],
        )?;

        // Migration: add duration_secs column if missing
        let has_duration: bool = self.conn
            .prepare("SELECT duration_secs FROM trainings LIMIT 1")
            .is_ok();
        if !has_duration {
            let _ = self.conn.execute(
                "ALTER TABLE trainings ADD COLUMN duration_secs INTEGER",
                [],
            );
        }

        // Migration: add pulse columns if missing
        let has_pulse: bool = self.conn
            .prepare("SELECT pulse_before FROM trainings LIMIT 1")
            .is_ok();
        if !has_pulse {
            let _ = self.conn.execute(
                "ALTER TABLE trainings ADD COLUMN pulse_before INTEGER",
                [],
            );
            let _ = self.conn.execute(
                "ALTER TABLE trainings ADD COLUMN pulse_after INTEGER",
                [],
            );
        }

        Ok(())
    }

    /// Add new training record
    pub fn add_training(&self, training: &Training) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO trainings (date, exercise, sets, reps, duration_secs, pulse_before, pulse_after, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                training.date.to_rfc3339(),
                training.exercise,
                training.sets,
                training.reps,
                training.duration_secs,
                training.pulse_before,
                training.pulse_after,
                training.notes,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all trainings
    pub fn get_trainings(&self) -> Result<Vec<Training>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, exercise, sets, reps, duration_secs, pulse_before, pulse_after, notes FROM trainings ORDER BY date DESC"
        )?;

        let trainings = stmt.query_map([], |row| {
            let date_str: String = row.get(1)?;
            Ok(Training {
                id: Some(row.get(0)?),
                date: DateTime::parse_from_rfc3339(&date_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                exercise: row.get(2)?,
                sets: row.get(3)?,
                reps: row.get(4)?,
                duration_secs: row.get(5)?,
                pulse_before: row.get(6)?,
                pulse_after: row.get(7)?,
                notes: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(trainings)
    }
}
