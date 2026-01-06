//! majowuji - Personal martial arts training tracker
//!
//! 无极 (wuji) - "limitless", the state of infinite potential

pub mod bot;
pub mod db;
pub mod exercises;
pub mod ml;
pub mod tips;
pub mod tui;

pub use db::Database;
pub use exercises::{Exercise, BASE_EXERCISES, get_base_exercises};
