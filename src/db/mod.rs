//! Database module - SQLite storage for training data

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

/// User record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub chat_id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_owner: bool,
}

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
    pub user_id: Option<i64>,        // Owner of this training record
}

/// Parse date string from database (supports RFC3339 and legacy "YYYY-MM-DD HH:MM:SS" format)
fn parse_date(date_str: &str) -> DateTime<Utc> {
    // Try RFC3339 first (new format with timezone)
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return dt.with_timezone(&Utc);
    }

    // Try legacy format without timezone (assume UTC)
    if let Ok(naive) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        return naive.and_utc();
    }

    // Fallback to epoch (1970-01-01) for truly invalid dates
    DateTime::UNIX_EPOCH
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
        // Users table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER UNIQUE NOT NULL,
                username TEXT,
                first_name TEXT,
                created_at TEXT NOT NULL,
                is_owner BOOLEAN DEFAULT FALSE
            )",
            [],
        )?;

        // Trainings table
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
                notes TEXT,
                user_id INTEGER REFERENCES users(id)
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

        // Migration: add user_id column if missing
        let has_user_id: bool = self.conn
            .prepare("SELECT user_id FROM trainings LIMIT 1")
            .is_ok();
        if !has_user_id {
            let _ = self.conn.execute(
                "ALTER TABLE trainings ADD COLUMN user_id INTEGER REFERENCES users(id)",
                [],
            );
        }

        Ok(())
    }

    // ==================== USER METHODS ====================

    /// Get or create user by chat_id (first user becomes owner)
    pub fn get_or_create_user(
        &self,
        chat_id: i64,
        username: Option<&str>,
        first_name: Option<&str>,
    ) -> Result<User> {
        // Check if user exists
        if let Some(user) = self.get_user_by_chat_id(chat_id)? {
            return Ok(user);
        }

        // First user becomes owner
        let is_owner = self.count_users()? == 0;

        // Create new user
        self.conn.execute(
            "INSERT INTO users (chat_id, username, first_name, created_at, is_owner) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chat_id, username, first_name, Utc::now().to_rfc3339(), is_owner],
        )?;

        self.get_user_by_chat_id(chat_id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to create user"))
    }

    /// Get user by chat_id
    pub fn get_user_by_chat_id(&self, chat_id: i64) -> Result<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, chat_id, username, first_name, created_at, is_owner FROM users WHERE chat_id = ?1"
        )?;

        let user = stmt.query_row([chat_id], |row| {
            let date_str: String = row.get(4)?;
            Ok(User {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                username: row.get(2)?,
                first_name: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&date_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                is_owner: row.get(5)?,
            })
        });

        match user {
            Ok(u) => Ok(Some(u)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Count total users
    pub fn count_users(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM users",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get owner user
    pub fn get_owner(&self) -> Result<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, chat_id, username, first_name, created_at, is_owner FROM users WHERE is_owner = 1"
        )?;

        let user = stmt.query_row([], |row| {
            let date_str: String = row.get(4)?;
            Ok(User {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                username: row.get(2)?,
                first_name: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&date_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                is_owner: row.get(5)?,
            })
        });

        match user {
            Ok(u) => Ok(Some(u)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // ==================== TRAINING METHODS ====================

    /// Add training record without user (CLI backward compatibility)
    pub fn add_training_cli(&self, training: &Training) -> Result<i64> {
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

    /// Add new training record for a user
    pub fn add_training(&self, training: &Training, user_id: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO trainings (date, exercise, sets, reps, duration_secs, pulse_before, pulse_after, notes, user_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                training.date.to_rfc3339(),
                training.exercise,
                training.sets,
                training.reps,
                training.duration_secs,
                training.pulse_before,
                training.pulse_after,
                training.notes,
                user_id,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get trainings for a specific user
    pub fn get_trainings_for_user(&self, user_id: i64) -> Result<Vec<Training>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, exercise, sets, reps, duration_secs, pulse_before, pulse_after, notes, user_id FROM trainings WHERE user_id = ?1 ORDER BY date DESC"
        )?;

        let trainings = stmt.query_map([user_id], |row| {
            let date_str: String = row.get(1)?;
            Ok(Training {
                id: Some(row.get(0)?),
                date: parse_date(&date_str),
                exercise: row.get(2)?,
                sets: row.get(3)?,
                reps: row.get(4)?,
                duration_secs: row.get(5)?,
                pulse_before: row.get(6)?,
                pulse_after: row.get(7)?,
                notes: row.get(8)?,
                user_id: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(trainings)
    }

    /// Get all trainings (for CLI/backward compatibility)
    pub fn get_trainings(&self) -> Result<Vec<Training>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, exercise, sets, reps, duration_secs, pulse_before, pulse_after, notes, user_id FROM trainings ORDER BY date DESC"
        )?;

        let trainings = stmt.query_map([], |row| {
            let date_str: String = row.get(1)?;
            Ok(Training {
                id: Some(row.get(0)?),
                date: parse_date(&date_str),
                exercise: row.get(2)?,
                sets: row.get(3)?,
                reps: row.get(4)?,
                duration_secs: row.get(5)?,
                pulse_before: row.get(6)?,
                pulse_after: row.get(7)?,
                notes: row.get(8)?,
                user_id: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(trainings)
    }

    /// Migrate existing trainings to owner (call after first user registration)
    pub fn migrate_trainings_to_owner(&self) -> Result<usize> {
        if let Some(owner) = self.get_owner()? {
            let affected = self.conn.execute(
                "UPDATE trainings SET user_id = ?1 WHERE user_id IS NULL",
                [owner.id],
            )?;
            Ok(affected)
        } else {
            Ok(0)
        }
    }
}
