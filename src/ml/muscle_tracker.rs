//! Muscle group load tracking for balanced training recommendations

use std::collections::HashMap;
use chrono::{DateTime, Local, Utc};
use crate::db::Training;
use crate::exercises::{MuscleGroup, find_exercise_by_name};

/// Load statistics for a single muscle group
#[derive(Debug, Clone)]
pub struct MuscleLoad {
    pub group: MuscleGroup,
    pub today_volume: i32,
    pub week_volume: i32,
    pub last_trained: Option<DateTime<Utc>>,
}

/// Tracks muscle group load from training history
pub struct MuscleTracker {
    loads: HashMap<MuscleGroup, MuscleLoad>,
}

impl MuscleTracker {
    /// Build tracker from training history
    pub fn from_trainings(trainings: &[Training]) -> Self {
        let mut loads: HashMap<MuscleGroup, MuscleLoad> = HashMap::new();

        // Initialize all muscle groups
        for group in MuscleGroup::all() {
            loads.insert(*group, MuscleLoad {
                group: *group,
                today_volume: 0,
                week_volume: 0,
                last_trained: None,
            });
        }

        let now = Local::now();
        let today = now.date_naive();
        let week_ago = today - chrono::Duration::days(7);

        for training in trainings {
            // Find exercise definition to get muscle groups
            let exercise = match find_exercise_by_name(&training.exercise) {
                Some(ex) => ex,
                None => continue, // Unknown exercise, skip
            };

            let training_date = training.date.with_timezone(&Local).date_naive();
            let is_today = training_date == today;
            let is_this_week = training_date >= week_ago;

            // Distribute reps to each muscle group the exercise targets
            for muscle_group in exercise.muscle_groups {
                if let Some(load) = loads.get_mut(muscle_group) {
                    if is_today {
                        load.today_volume += training.reps;
                    }
                    if is_this_week {
                        load.week_volume += training.reps;
                    }

                    // Update last trained time
                    if load.last_trained.is_none() || load.last_trained.unwrap() < training.date {
                        load.last_trained = Some(training.date);
                    }
                }
            }
        }

        Self { loads }
    }

    /// Get load for a specific muscle group
    pub fn get_load(&self, group: &MuscleGroup) -> Option<&MuscleLoad> {
        self.loads.get(group)
    }

    /// Get all loads sorted by today's volume (ascending = least worked first)
    pub fn get_loads_sorted(&self) -> Vec<&MuscleLoad> {
        let mut loads: Vec<_> = self.loads.values().collect();
        loads.sort_by_key(|l| l.today_volume);
        loads
    }

    /// Get underworked muscle groups (least volume today, excluding FullBody)
    pub fn get_underworked_groups(&self, limit: usize) -> Vec<MuscleGroup> {
        self.get_loads_sorted()
            .iter()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .take(limit)
            .map(|l| l.group)
            .collect()
    }

    /// Calculate balance score (0-100%)
    /// 100% = perfectly balanced across all groups
    pub fn get_balance_score(&self) -> f32 {
        let volumes: Vec<i32> = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|l| l.week_volume)
            .collect();

        if volumes.is_empty() {
            return 0.0;
        }

        let total: i32 = volumes.iter().sum();
        if total == 0 {
            return 0.0;
        }

        let target = total as f32 / volumes.len() as f32;
        let variance: f32 = volumes.iter()
            .map(|v| (*v as f32 - target).powi(2))
            .sum::<f32>() / volumes.len() as f32;

        let std_dev = variance.sqrt();
        let cv = if target > 0.0 { std_dev / target } else { 0.0 };

        // Convert coefficient of variation to score (lower CV = higher score)
        // CV of 0 = 100%, CV of 1+ = ~0%
        ((1.0 - cv.min(1.0)) * 100.0).max(0.0)
    }

    /// Get weekly report for /balance command
    pub fn get_weekly_report(&self) -> Vec<(MuscleGroup, i32, &'static str)> {
        let max_volume = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|l| l.week_volume)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut report: Vec<_> = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|load| {
                let ratio = load.week_volume as f32 / max_volume as f32;
                let bar = match ratio {
                    r if r >= 0.75 => "[++++]",
                    r if r >= 0.50 => "[+++.]",
                    r if r >= 0.25 => "[++..]",
                    r if r > 0.0 => "[+...]",
                    _ => "[....]",
                };
                (load.group, load.week_volume, bar)
            })
            .collect();

        report.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by volume descending
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tracker() {
        let tracker = MuscleTracker::from_trainings(&[]);
        assert_eq!(tracker.get_balance_score(), 0.0);
        assert_eq!(tracker.get_underworked_groups(3).len(), 3);
    }
}
