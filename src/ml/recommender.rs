//! Exercise recommendation engine based on muscle group balance

use chrono::Utc;
use crate::db::Training;
use crate::exercises::{Exercise, get_base_exercises};
use super::muscle_tracker::MuscleTracker;

/// A recommendation with explanation
#[derive(Debug, Clone)]
pub struct Recommendation {
    pub exercise: &'static Exercise,
    pub reason: String,
    pub confidence: f32,
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

    /// Get best exercise recommendation
    pub fn get_recommendation(&self) -> Option<Recommendation> {
        let exercises = get_base_exercises();

        // Strategy 1: Find exercises targeting underworked muscle groups
        let underworked = self.tracker.get_underworked_groups(5);
        let mut candidates: Vec<(&'static Exercise, f32, String)> = Vec::new();

        for exercise in exercises {
            // Check rest time (skip if done recently)
            let hours_since = self.hours_since_exercise(exercise.name);
            if hours_since < 1.0 {
                continue; // Need at least 1 hour rest
            }

            // Check if exercise targets any underworked muscle
            let targets_underworked: Vec<_> = exercise.muscle_groups
                .iter()
                .filter(|mg| underworked.contains(mg))
                .collect();

            if !targets_underworked.is_empty() {
                // Calculate score based on how many underworked muscles it targets
                let score = targets_underworked.len() as f32 / exercise.muscle_groups.len() as f32;
                // Bonus for longer rest
                let rest_bonus = (hours_since / 24.0).min(0.5);
                let final_score = score + rest_bonus;

                // Build reason string
                let muscle_names: Vec<_> = targets_underworked
                    .iter()
                    .map(|mg| mg.name_ru())
                    .collect();
                let reason = format!("{} мало работали сегодня", muscle_names.join(", "));

                candidates.push((exercise, final_score, reason));
            }
        }

        // Sort by score (highest first)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // If we have candidates targeting underworked muscles, use them
        if let Some((exercise, score, reason)) = candidates.into_iter().next() {
            return Some(Recommendation {
                exercise,
                reason,
                confidence: score,
            });
        }

        // Strategy 2: Fallback - recommend exercise with longest rest time
        let mut fallback: Vec<_> = exercises
            .iter()
            .map(|ex| (ex, self.hours_since_exercise(ex.name)))
            .filter(|(_, hours)| *hours >= 1.0) // At least 1 hour rest
            .collect();

        fallback.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        fallback.into_iter().next().map(|(exercise, hours)| {
            let reason = if hours == f32::MAX {
                "ещё не делали".to_string()
            } else {
                format!("отдохнули {:.0}ч", hours)
            };
            Recommendation {
                exercise,
                reason,
                confidence: 0.5,
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
