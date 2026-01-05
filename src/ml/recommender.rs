//! Exercise recommendation engine based on muscle group balance

use chrono::{Local, Utc};
use crate::db::Training;
use crate::exercises::{Exercise, get_base_exercises, get_all_exercises};
use super::muscle_tracker::MuscleTracker;

/// A recommendation with explanation
#[derive(Debug, Clone)]
pub struct Recommendation {
    pub exercise: &'static Exercise,
    pub reason: String,
    pub confidence: f32,
    pub is_bonus: bool,
}

/// Exercise recommendation engine
pub struct Recommender {
    tracker: MuscleTracker,
    trainings: Vec<Training>,
}

impl Recommender {
    /// Create recommender from training history
    pub fn new(trainings: Vec<Training>) -> Self {
        let tracker = MuscleTracker::from_trainings(&trainings);
        Self { tracker, trainings }
    }

    /// Check if all base exercises were done today
    fn base_program_done_today(&self) -> bool {
        let today = Local::now().date_naive();
        let base_exercises = get_base_exercises();

        for exercise in base_exercises {
            let done_today = self.trainings.iter().any(|t| {
                t.exercise == exercise.name &&
                t.date.with_timezone(&Local).date_naive() == today
            });
            if !done_today {
                return false;
            }
        }
        true
    }

    /// Get best exercise recommendation
    pub fn get_recommendation(&self) -> Option<Recommendation> {
        // Check if base program is done today
        if self.base_program_done_today() {
            return self.get_bonus_recommendation();
        }

        // Recommend from base exercises
        self.get_base_recommendation()
    }

    /// Recommend base exercise
    fn get_base_recommendation(&self) -> Option<Recommendation> {
        let exercises = get_base_exercises();
        let today = Local::now().date_naive();

        // Find base exercises not done today, prioritize by muscle balance
        let underworked = self.tracker.get_underworked_groups(5);
        let mut candidates: Vec<(&'static Exercise, f32, String)> = Vec::new();

        for exercise in exercises {
            // Skip if done today
            let done_today = self.trainings.iter().any(|t| {
                t.exercise == exercise.name &&
                t.date.with_timezone(&Local).date_naive() == today
            });
            if done_today {
                continue;
            }

            // Check rest time
            let hours_since = self.hours_since_exercise(exercise.name);
            if hours_since < 1.0 {
                continue;
            }

            // Score by underworked muscles
            let targets_underworked: Vec<_> = exercise.muscle_groups
                .iter()
                .filter(|mg| underworked.contains(mg))
                .collect();

            let score = if !targets_underworked.is_empty() {
                targets_underworked.len() as f32 / exercise.muscle_groups.len() as f32 + 0.5
            } else {
                0.3
            };

            let reason = if !targets_underworked.is_empty() {
                let names: Vec<_> = targets_underworked.iter().map(|mg| mg.name_ru()).collect();
                format!("{} мало работали", names.join(", "))
            } else if hours_since == f32::MAX {
                "ещё не делали".to_string()
            } else {
                format!("отдохнули {:.0}ч", hours_since)
            };

            candidates.push((exercise, score, reason));
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        candidates.into_iter().next().map(|(exercise, score, reason)| {
            Recommendation {
                exercise,
                reason,
                confidence: score,
                is_bonus: false,
            }
        })
    }

    /// Recommend bonus exercise from the book
    fn get_bonus_recommendation(&self) -> Option<Recommendation> {
        let all_exercises = get_all_exercises();
        let underworked = self.tracker.get_underworked_groups(5);

        // Filter to non-base exercises that target underworked muscles
        let mut candidates: Vec<(&'static Exercise, f32, String)> = Vec::new();

        for exercise in all_exercises {
            if exercise.is_base {
                continue;
            }

            let hours_since = self.hours_since_exercise(exercise.name);
            if hours_since < 1.0 {
                continue;
            }

            let targets_underworked: Vec<_> = exercise.muscle_groups
                .iter()
                .filter(|mg| underworked.contains(mg))
                .collect();

            if !targets_underworked.is_empty() {
                let score = targets_underworked.len() as f32;
                let names: Vec<_> = targets_underworked.iter().map(|mg| mg.name_ru()).collect();
                let reason = format!("🎁 Бонус! {} нужна нагрузка", names.join(", "));
                candidates.push((exercise, score, reason));
            }
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        candidates.into_iter().next().map(|(exercise, score, reason)| {
            Recommendation {
                exercise,
                reason,
                confidence: score,
                is_bonus: true,
            }
        })
    }

    /// Get hours since last time this exercise was done
    fn hours_since_exercise(&self, exercise_name: &str) -> f32 {
        let last = self.trainings
            .iter()
            .find(|t| t.exercise == exercise_name);

        match last {
            Some(t) => {
                let now = Utc::now();
                let diff = now - t.date;
                diff.num_minutes() as f32 / 60.0
            }
            None => f32::MAX, // Never done = infinite rest
        }
    }

    /// Get balance score (0-100%)
    pub fn get_balance_score(&self) -> f32 {
        self.tracker.get_balance_score()
    }

    /// Get weekly balance report for /balance command
    pub fn get_balance_report(&self) -> String {
        let score = self.tracker.get_balance_score();
        let report = self.tracker.get_weekly_report();

        let mut lines = vec![
            format!("Баланс за неделю: {:.0}%\n", score),
        ];

        for (group, volume, bar) in report {
            let indicator = if volume == 0 { " ← нужно больше" } else { "" };
            lines.push(format!("{} {}: {} повторов{}", bar, group.name_ru(), volume, indicator));
        }

        lines.join("\n")
    }

    /// Get tracker reference for detailed queries
    pub fn tracker(&self) -> &MuscleTracker {
        &self.tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_recommender() {
        let recommender = Recommender::new(vec![]);
        // Should recommend something even with no history
        let rec = recommender.get_recommendation();
        assert!(rec.is_some());
    }
}
