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
                notes TEXT
            )",
            [],
        )?;
        Ok(())
    }

    /// Add new training record
    pub fn add_training(&self, training: &Training) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO trainings (date, exercise, sets, reps, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                training.date.to_rfc3339(),
                training.exercise,
                training.sets,
                training.reps,
                training.notes,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all trainings
    pub fn get_trainings(&self) -> Result<Vec<Training>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, exercise, sets, reps, notes FROM trainings ORDER BY date DESC"
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
                notes: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(trainings)
    }
}
