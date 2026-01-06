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
pub(crate) fn parse_date(date_str: &str) -> DateTime<Utc> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    fn create_test_db() -> Database {
        Database::open(":memory:").unwrap()
    }

    fn create_test_training(exercise: &str, reps: i32) -> Training {
        Training {
            id: None,
            date: Utc::now(),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: Some(30),
            pulse_before: Some(80),
            pulse_after: Some(120),
            notes: None,
            user_id: None,
        }
    }

    // ==================== parse_date tests ====================

    #[test]
    fn test_parse_date_rfc3339() {
        let date_str = "2026-01-06T12:30:00+00:00";
        let parsed = parse_date(date_str);
        assert_eq!(parsed.year(), 2026);
        assert_eq!(parsed.month(), 1);
        assert_eq!(parsed.day(), 6);
    }

    #[test]
    fn test_parse_date_rfc3339_with_timezone() {
        let date_str = "2026-01-06T15:30:00+03:00";
        let parsed = parse_date(date_str);
        // Should be converted to UTC: 15:30 + 03:00 = 12:30 UTC
        assert_eq!(parsed.hour(), 12);
    }

    #[test]
    fn test_parse_date_legacy_format() {
        let date_str = "2026-01-05 14:12:29";
        let parsed = parse_date(date_str);
        assert_eq!(parsed.year(), 2026);
        assert_eq!(parsed.month(), 1);
        assert_eq!(parsed.day(), 5);
        assert_eq!(parsed.hour(), 14);
        assert_eq!(parsed.minute(), 12);
    }

    #[test]
    fn test_parse_date_invalid_fallback_to_epoch() {
        let date_str = "invalid-date";
        let parsed = parse_date(date_str);
        assert_eq!(parsed, DateTime::UNIX_EPOCH);
    }

    // ==================== Database tests ====================

    #[test]
    fn test_database_open_in_memory() {
        let db = create_test_db();
        assert_eq!(db.count_users().unwrap(), 0);
    }

    #[test]
    fn test_get_or_create_user_new() {
        let db = create_test_db();
        let user = db.get_or_create_user(12345, Some("test_user"), Some("Test")).unwrap();
        assert_eq!(user.chat_id, 12345);
        assert_eq!(user.username, Some("test_user".to_string()));
        assert_eq!(user.first_name, Some("Test".to_string()));
    }

    #[test]
    fn test_get_or_create_user_existing() {
        let db = create_test_db();
        let user1 = db.get_or_create_user(12345, Some("user1"), None).unwrap();
        let user2 = db.get_or_create_user(12345, Some("user2"), None).unwrap();
        // Should return same user
        assert_eq!(user1.id, user2.id);
        // Username should not change
        assert_eq!(user2.username, Some("user1".to_string()));
    }

    #[test]
    fn test_first_user_is_owner() {
        let db = create_test_db();
        let user1 = db.get_or_create_user(111, None, None).unwrap();
        assert!(user1.is_owner, "First user should be owner");

        let user2 = db.get_or_create_user(222, None, None).unwrap();
        assert!(!user2.is_owner, "Second user should not be owner");
    }

    #[test]
    fn test_get_user_by_chat_id_found() {
        let db = create_test_db();
        db.get_or_create_user(12345, Some("test"), None).unwrap();

        let user = db.get_user_by_chat_id(12345).unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().chat_id, 12345);
    }

    #[test]
    fn test_get_user_by_chat_id_not_found() {
        let db = create_test_db();
        let user = db.get_user_by_chat_id(99999).unwrap();
        assert!(user.is_none());
    }

    #[test]
    fn test_count_users() {
        let db = create_test_db();
        assert_eq!(db.count_users().unwrap(), 0);

        db.get_or_create_user(111, None, None).unwrap();
        assert_eq!(db.count_users().unwrap(), 1);

        db.get_or_create_user(222, None, None).unwrap();
        assert_eq!(db.count_users().unwrap(), 2);

        // Same user again - should not increase count
        db.get_or_create_user(111, None, None).unwrap();
        assert_eq!(db.count_users().unwrap(), 2);
    }

    #[test]
    fn test_get_owner() {
        let db = create_test_db();

        // No owner initially
        assert!(db.get_owner().unwrap().is_none());

        // First user becomes owner
        db.get_or_create_user(111, Some("owner"), None).unwrap();
        let owner = db.get_owner().unwrap();
        assert!(owner.is_some());
        assert_eq!(owner.unwrap().chat_id, 111);
    }

    #[test]
    fn test_add_training_cli() {
        let db = create_test_db();
        let training = create_test_training("отжимания", 15);

        let id = db.add_training_cli(&training).unwrap();
        assert!(id > 0);

        let trainings = db.get_trainings().unwrap();
        assert_eq!(trainings.len(), 1);
        assert_eq!(trainings[0].exercise, "отжимания");
        assert_eq!(trainings[0].reps, 15);
    }

    #[test]
    fn test_add_training_with_user() {
        let db = create_test_db();
        let user = db.get_or_create_user(12345, None, None).unwrap();
        let training = create_test_training("планка", 1);

        let id = db.add_training(&training, user.id).unwrap();
        assert!(id > 0);

        let trainings = db.get_trainings_for_user(user.id).unwrap();
        assert_eq!(trainings.len(), 1);
        assert_eq!(trainings[0].user_id, Some(user.id));
    }

    #[test]
    fn test_get_trainings_for_user_empty() {
        let db = create_test_db();
        let user = db.get_or_create_user(12345, None, None).unwrap();

        let trainings = db.get_trainings_for_user(user.id).unwrap();
        assert!(trainings.is_empty());
    }

    #[test]
    fn test_get_trainings_for_user_filters_by_user() {
        let db = create_test_db();
        let user1 = db.get_or_create_user(111, None, None).unwrap();
        let user2 = db.get_or_create_user(222, None, None).unwrap();

        db.add_training(&create_test_training("упр1", 10), user1.id).unwrap();
        db.add_training(&create_test_training("упр2", 20), user2.id).unwrap();
        db.add_training(&create_test_training("упр3", 30), user1.id).unwrap();

        let user1_trainings = db.get_trainings_for_user(user1.id).unwrap();
        assert_eq!(user1_trainings.len(), 2);

        let user2_trainings = db.get_trainings_for_user(user2.id).unwrap();
        assert_eq!(user2_trainings.len(), 1);
    }

    #[test]
    fn test_trainings_ordered_desc() {
        let db = create_test_db();
        let user = db.get_or_create_user(12345, None, None).unwrap();

        // Add trainings (they get same timestamp in tests, but order should be by insert)
        db.add_training(&create_test_training("first", 1), user.id).unwrap();
        db.add_training(&create_test_training("second", 2), user.id).unwrap();

        let trainings = db.get_trainings_for_user(user.id).unwrap();
        // Last added should be first (DESC order)
        assert_eq!(trainings[0].exercise, "second");
    }

    #[test]
    fn test_migrate_trainings_to_owner() {
        let db = create_test_db();

        // Add CLI trainings (no user_id)
        db.add_training_cli(&create_test_training("old1", 10)).unwrap();
        db.add_training_cli(&create_test_training("old2", 20)).unwrap();

        // Create owner
        let owner = db.get_or_create_user(12345, None, None).unwrap();

        // Migrate
        let migrated = db.migrate_trainings_to_owner().unwrap();
        assert_eq!(migrated, 2);

        // Check owner now has those trainings
        let trainings = db.get_trainings_for_user(owner.id).unwrap();
        assert_eq!(trainings.len(), 2);
    }

    #[test]
    fn test_migrate_trainings_no_owner() {
        let db = create_test_db();

        // Add CLI trainings
        db.add_training_cli(&create_test_training("old", 10)).unwrap();

        // No owner yet
        let migrated = db.migrate_trainings_to_owner().unwrap();
        assert_eq!(migrated, 0);
    }

    #[test]
    fn test_training_pulse_fields() {
        let db = create_test_db();
        let user = db.get_or_create_user(12345, None, None).unwrap();

        let training = Training {
            id: None,
            date: Utc::now(),
            exercise: "test".to_string(),
            sets: 1,
            reps: 10,
            duration_secs: Some(45),
            pulse_before: Some(75),
            pulse_after: Some(130),
            notes: Some("test note".to_string()),
            user_id: None,
        };

        db.add_training(&training, user.id).unwrap();

        let trainings = db.get_trainings_for_user(user.id).unwrap();
        assert_eq!(trainings[0].pulse_before, Some(75));
        assert_eq!(trainings[0].pulse_after, Some(130));
        assert_eq!(trainings[0].duration_secs, Some(45));
        assert_eq!(trainings[0].notes, Some("test note".to_string()));
    }
}
