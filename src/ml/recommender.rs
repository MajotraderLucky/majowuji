//! Exercise recommendation engine based on muscle group balance

use chrono::{Local, Utc};
use crate::db::Training;
use crate::exercises::{Exercise, find_exercise_by_name, get_base_exercises, get_all_exercises};
use super::muscle_tracker::MuscleTracker;

/// A recommendation with explanation
#[derive(Debug, Clone)]
pub struct Recommendation {
    pub exercise: &'static Exercise,
    pub reason: String,
    pub confidence: f32,
    pub is_bonus: bool,
    /// Detailed description for bonus exercises
    pub detailed_description: Option<String>,
    /// Focus cues for muscle awareness
    pub focus_cues: Option<String>,
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
            // Skip if done today (exact exercise)
            let done_today = self.trainings.iter().any(|t| {
                t.exercise == exercise.name &&
                t.date.with_timezone(&Local).date_naive() == today
            });
            if done_today {
                continue;
            }

            // Skip if same category exercise was done today (e.g., both push exercises)
            let same_category_done = self.trainings.iter().any(|t| {
                if t.date.with_timezone(&Local).date_naive() != today {
                    return false;
                }
                if let Some(done_ex) = find_exercise_by_name(&t.exercise) {
                    done_ex.category == exercise.category
                } else {
                    false
                }
            });
            if same_category_done {
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
                detailed_description: None,
                focus_cues: None,
            }
        })
    }

    /// Recommend bonus exercise with smart diversity logic
    /// Priority 1: Never done + targets underworked muscles
    /// Priority 2: Never done (any)
    /// Priority 3: All done → recommend for balance (sorted by recency + underworked)
    fn get_bonus_recommendation(&self) -> Option<Recommendation> {
        let bonus_exercises: Vec<_> = get_all_exercises()
            .into_iter()
            .filter(|e| !e.is_base)
            .collect();

        let underworked = self.tracker.get_underworked_groups(5);

        // Helper: check if exercise targets underworked muscles
        let targets_underworked = |ex: &Exercise| -> bool {
            ex.muscle_groups.iter().any(|mg| underworked.contains(mg))
        };

        // Helper: count underworked muscles targeted
        let underworked_count = |ex: &Exercise| -> usize {
            ex.muscle_groups.iter().filter(|mg| underworked.contains(mg)).count()
        };

        // Group 1: Never done + targets underworked muscles
        let never_done_underworked: Vec<_> = bonus_exercises.iter()
            .filter(|e| !self.ever_done(e.name) && targets_underworked(e))
            .collect();

        if !never_done_underworked.is_empty() {
            // Sort by number of underworked muscles targeted
            let mut sorted = never_done_underworked;
            sorted.sort_by(|a, b| underworked_count(b).cmp(&underworked_count(a)));

            let exercise = sorted[0];
            let muscle_names: Vec<_> = exercise.muscle_groups
                .iter()
                .filter(|mg| underworked.contains(mg))
                .map(|mg| mg.name_ru())
                .collect();

            return Some(Recommendation {
                exercise,
                reason: format!("Новое упражнение! {} нужна нагрузка", muscle_names.join(", ")),
                confidence: 1.0,
                is_bonus: true,
                detailed_description: exercise.description.map(|s| s.to_string()),
                focus_cues: exercise.focus_cues.map(|s| s.to_string()),
            });
        }

        // Group 2: Never done (any bonus exercise)
        let never_done_any: Vec<_> = bonus_exercises.iter()
            .filter(|e| !self.ever_done(e.name))
            .collect();

        if !never_done_any.is_empty() {
            // Prefer those targeting underworked muscles, then by variety
            let mut sorted = never_done_any;
            sorted.sort_by(|a, b| underworked_count(b).cmp(&underworked_count(a)));

            let exercise = sorted[0];
            return Some(Recommendation {
                exercise,
                reason: "Новое упражнение для разнообразия".to_string(),
                confidence: 0.9,
                is_bonus: true,
                detailed_description: exercise.description.map(|s| s.to_string()),
                focus_cues: exercise.focus_cues.map(|s| s.to_string()),
            });
        }

        // Group 3: All done - cycle back, prioritize by:
        // 1. Targets underworked muscles
        // 2. Days since last done (longer = better)
        let mut all_with_score: Vec<_> = bonus_exercises.iter()
            .filter(|e| {
                // Skip if done recently (within 1 hour)
                self.hours_since_exercise(e.name) >= 1.0
            })
            .map(|e| {
                let days = self.days_since_exercise(e.name).unwrap_or(0);
                let underworked_score = underworked_count(e) as f32 * 10.0;
                let recency_score = (days as f32).min(30.0); // Cap at 30 days
                let total_score = underworked_score + recency_score;
                (*e, total_score)
            })
            .collect();

        all_with_score.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        all_with_score.into_iter().next().map(|(exercise, score)| {
            let days = self.days_since_exercise(exercise.name).unwrap_or(0);
            let muscle_names: Vec<_> = exercise.muscle_groups
                .iter()
                .filter(|mg| underworked.contains(mg))
                .map(|mg| mg.name_ru())
                .collect();

            let reason = if !muscle_names.is_empty() {
                format!("{} нужна нагрузка (последний раз {} дн. назад)",
                    muscle_names.join(", "), days)
            } else {
                format!("Давно не делали ({} дн. назад)", days)
            };

            Recommendation {
                exercise,
                reason,
                confidence: score / 50.0, // Normalize to ~0-1 range
                is_bonus: true,
                detailed_description: exercise.description.map(|s| s.to_string()),
                focus_cues: exercise.focus_cues.map(|s| s.to_string()),
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

    /// Check if exercise was ever done (any time in history)
    fn ever_done(&self, exercise_name: &str) -> bool {
        self.trainings.iter().any(|t| t.exercise == exercise_name)
    }

    /// Get days since last time exercise was done
    fn days_since_exercise(&self, exercise_name: &str) -> Option<i64> {
        self.trainings
            .iter()
            .find(|t| t.exercise == exercise_name)
            .map(|t| (Utc::now() - t.date).num_days())
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
    use crate::exercises::Category;

    fn create_training(exercise: &str, reps: i32) -> Training {
        Training {
            id: None,
            date: Utc::now(),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    fn create_training_hours_ago(exercise: &str, reps: i32, hours_ago: i64) -> Training {
        Training {
            id: None,
            date: Utc::now() - chrono::Duration::hours(hours_ago),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    #[test]
    fn test_empty_recommender() {
        let recommender = Recommender::new(vec![]);
        // Should recommend something even with no history
        let rec = recommender.get_recommendation();
        assert!(rec.is_some());
    }

    #[test]
    fn test_recommendation_is_base_exercise() {
        let recommender = Recommender::new(vec![]);
        let rec = recommender.get_recommendation().unwrap();
        // Without history, should recommend a base exercise
        assert!(rec.exercise.is_base);
        assert!(!rec.is_bonus);
    }

    #[test]
    fn test_skip_done_today() {
        let trainings = vec![
            create_training("отжимания на кулаках", 20),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation().unwrap();

        // Should not recommend the same exercise done today
        assert_ne!(rec.exercise.name, "отжимания на кулаках");
    }

    #[test]
    fn test_skip_same_category_today() {
        // If pushups on fists done today, should not recommend pushups with handles
        let trainings = vec![
            create_training("отжимания на кулаках", 20),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation().unwrap();

        // Should not recommend another Push exercise
        assert_ne!(rec.exercise.category, Category::Push,
            "Should not recommend {} (Push category) when pushups already done", rec.exercise.name);
    }

    #[test]
    fn test_hours_since_never_done() {
        let recommender = Recommender::new(vec![]);
        let hours = recommender.hours_since_exercise("отжимания на кулаках");
        assert_eq!(hours, f32::MAX);
    }

    #[test]
    fn test_hours_since_recently_done() {
        let trainings = vec![
            create_training_hours_ago("отжимания на кулаках", 20, 2),
        ];
        let recommender = Recommender::new(trainings);
        let hours = recommender.hours_since_exercise("отжимания на кулаках");

        // Should be approximately 2 hours
        assert!(hours > 1.9 && hours < 2.1, "Expected ~2 hours, got {}", hours);
    }

    #[test]
    fn test_rest_time_enforcement() {
        // Exercise done 30 minutes ago - should be skipped
        let trainings = vec![
            create_training("отжимания на кулаках", 20),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation().unwrap();

        // Recent exercise should not be recommended
        assert_ne!(rec.exercise.name, "отжимания на кулаках",
            "Exercise done < 1 hour ago should not be recommended");
    }

    #[test]
    fn test_recommendation_has_reason() {
        let recommender = Recommender::new(vec![]);
        let rec = recommender.get_recommendation().unwrap();
        assert!(!rec.reason.is_empty());
    }

    #[test]
    fn test_recommendation_has_confidence() {
        let recommender = Recommender::new(vec![]);
        let rec = recommender.get_recommendation().unwrap();
        assert!(rec.confidence > 0.0);
    }

    #[test]
    fn test_balance_score_passthrough() {
        let recommender = Recommender::new(vec![]);
        let score = recommender.get_balance_score();
        // Empty history = 0% balance
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_balance_report_format() {
        let trainings = vec![
            create_training("отжимания на кулаках", 30),
        ];
        let recommender = Recommender::new(trainings);
        let report = recommender.get_balance_report();

        // Report should contain balance percentage
        assert!(report.contains("Баланс за неделю:"));
        assert!(report.contains("%"));

        // Should contain muscle group data
        assert!(report.contains("повторов"));
    }

    #[test]
    fn test_balance_report_shows_underworked() {
        let trainings = vec![
            create_training("отжимания на кулаках", 30),
        ];
        let recommender = Recommender::new(trainings);
        let report = recommender.get_balance_report();

        // Non-trained muscles should show "нужно больше"
        assert!(report.contains("нужно больше"),
            "Underworked muscles should be indicated");
    }

    #[test]
    fn test_tracker_accessor() {
        let recommender = Recommender::new(vec![]);
        let tracker = recommender.tracker();
        // Should return valid tracker reference
        assert_eq!(tracker.get_balance_score(), 0.0);
    }

    #[test]
    fn test_base_program_not_done_with_partial() {
        // Only one exercise done - base program not complete
        let trainings = vec![
            create_training("отжимания на кулаках", 20),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation().unwrap();

        // Should still recommend base exercises, not bonus
        assert!(!rec.is_bonus);
    }

    #[test]
    fn test_ever_done_true() {
        let trainings = vec![
            create_training_hours_ago("впусти меня", 10, 48),
        ];
        let recommender = Recommender::new(trainings);
        assert!(recommender.ever_done("впусти меня"));
    }

    #[test]
    fn test_ever_done_false() {
        let recommender = Recommender::new(vec![]);
        assert!(!recommender.ever_done("впусти меня"));
    }

    #[test]
    fn test_days_since_exercise_none() {
        let recommender = Recommender::new(vec![]);
        assert!(recommender.days_since_exercise("впусти меня").is_none());
    }

    #[test]
    fn test_days_since_exercise_some() {
        let trainings = vec![
            create_training_hours_ago("впусти меня", 10, 48), // 2 days ago
        ];
        let recommender = Recommender::new(trainings);
        let days = recommender.days_since_exercise("впусти меня").unwrap();
        assert!(days >= 1 && days <= 3, "Expected ~2 days, got {}", days);
    }

    fn create_training_local_today(exercise: &str, reps: i32, hours_ago: i64) -> Training {
        // Create training that is definitely today in local timezone
        // and hours_ago hours in the past for rest time checks
        let training_time = Local::now() - chrono::Duration::hours(hours_ago);

        Training {
            id: None,
            date: training_time.with_timezone(&Utc),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    #[test]
    fn test_bonus_recommendation_has_focus_cues() {
        // Create trainings for all base exercises to trigger bonus recommendation
        // Base exercises: отжимания на кулаках, отжимания с ручками, пресс складной нож,
        //                 стойка на локтях, приседания с ударами, тайцзи бой с тенью
        let trainings = vec![
            create_training_local_today("отжимания на кулаках", 20, 2),
            create_training_local_today("отжимания с ручками", 20, 2),
            create_training_local_today("пресс складной нож", 20, 2),
            create_training_local_today("стойка на локтях", 60, 2),
            create_training_local_today("приседания с ударами", 30, 2),
            create_training_local_today("тайцзи бой с тенью", 60, 2),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation();

        assert!(rec.is_some(), "Should have a recommendation");
        let rec = rec.unwrap();
        assert!(rec.is_bonus, "Should be a bonus recommendation");
        // Bonus recommendations should have focus_cues
        assert!(rec.focus_cues.is_some(), "Bonus should have focus_cues");
    }

    #[test]
    fn test_bonus_prioritizes_never_done() {
        // Do all base + some bonus exercises
        // Base exercises: отжимания на кулаках, отжимания с ручками, пресс складной нож,
        //                 стойка на локтях, приседания с ударами, тайцзи бой с тенью
        let trainings = vec![
            create_training_local_today("отжимания на кулаках", 20, 2),
            create_training_local_today("отжимания с ручками", 20, 2),
            create_training_local_today("пресс складной нож", 20, 2),
            create_training_local_today("стойка на локтях", 60, 2),
            create_training_local_today("приседания с ударами", 30, 2),
            create_training_local_today("тайцзи бой с тенью", 60, 2),
            // Some bonus exercises done (2 hours ago to pass rest time check)
            create_training_hours_ago("впусти меня", 10, 2),
            create_training_hours_ago("подъём на носки", 20, 2),
        ];
        let recommender = Recommender::new(trainings);
        let rec = recommender.get_recommendation().unwrap();

        assert!(rec.is_bonus);
        // Should NOT recommend already done bonus exercises
        assert_ne!(rec.exercise.name, "впусти меня");
        assert_ne!(rec.exercise.name, "подъём на носки");
    }
}
