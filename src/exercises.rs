//! Exercise definitions - база упражнений

use serde::{Deserialize, Serialize};

/// Muscle groups for tracking training balance
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MuscleGroup {
    Chest,      // Грудные
    Shoulders,  // Плечи (дельты)
    Triceps,    // Трицепс
    Back,       // Спина
    Biceps,     // Бицепс
    Core,       // Кор (пресс, косые)
    Glutes,     // Ягодицы
    Quads,      // Квадрицепсы
    Hamstrings, // Бицепс бедра
    Calves,     // Икры
    FullBody,   // Всё тело (формы, тайцзи)
}

impl MuscleGroup {
    pub fn name_ru(&self) -> &'static str {
        match self {
            MuscleGroup::Chest => "грудные",
            MuscleGroup::Shoulders => "плечи",
            MuscleGroup::Triceps => "трицепс",
            MuscleGroup::Back => "спина",
            MuscleGroup::Biceps => "бицепс",
            MuscleGroup::Core => "кор",
            MuscleGroup::Glutes => "ягодицы",
            MuscleGroup::Quads => "квадрицепсы",
            MuscleGroup::Hamstrings => "бицепс бедра",
            MuscleGroup::Calves => "икры",
            MuscleGroup::FullBody => "всё тело",
        }
    }

    /// All muscle groups for iteration
    pub fn all() -> &'static [MuscleGroup] {
        &[
            MuscleGroup::Chest,
            MuscleGroup::Shoulders,
            MuscleGroup::Triceps,
            MuscleGroup::Back,
            MuscleGroup::Biceps,
            MuscleGroup::Core,
            MuscleGroup::Glutes,
            MuscleGroup::Quads,
            MuscleGroup::Hamstrings,
            MuscleGroup::Calves,
            MuscleGroup::FullBody,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct Exercise {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub muscle_groups: &'static [MuscleGroup],
    pub is_base: bool,
    pub is_timed: bool, // true = на время (планка), false = на повторы (отжимания)
    pub description: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Category {
    Push,      // Отжимания, жимы
    Pull,      // Подтягивания, тяги
    Core,      // Пресс, планка
    Legs,      // Ноги, приседания
    Taiji,     // Тайцзицюань
    Strikes,   // Удары
}

impl Category {
    pub fn emoji(&self) -> &'static str {
        match self {
            Category::Push => "💪",
            Category::Pull => "🏋️",
            Category::Core => "🎯",
            Category::Legs => "🦵",
            Category::Taiji => "☯",
            Category::Strikes => "👊",
        }
    }
}

/// Базовые упражнения (ежечасные)
pub const BASE_EXERCISES: &[Exercise] = &[
    Exercise {
        id: "pushups_fist",
        name: "отжимания на кулаках",
        category: Category::Push,
        muscle_groups: &[MuscleGroup::Chest, MuscleGroup::Triceps, MuscleGroup::Shoulders, MuscleGroup::Core],
        is_base: true,
        is_timed: false,
        description: None,
    },
    Exercise {
        id: "pushups_handles",
        name: "отжимания с ручками",
        category: Category::Push,
        muscle_groups: &[MuscleGroup::Chest, MuscleGroup::Triceps, MuscleGroup::Shoulders, MuscleGroup::Core],
        is_base: true,
        is_timed: false,
        description: None,
    },
    Exercise {
        id: "jackknife",
        name: "пресс складной нож",
        category: Category::Core,
        muscle_groups: &[MuscleGroup::Core],
        is_base: true,
        is_timed: false,
        description: None,
    },
    Exercise {
        id: "plank_elbows",
        name: "стойка на локтях",
        category: Category::Core,
        muscle_groups: &[MuscleGroup::Core, MuscleGroup::Shoulders],
        is_base: true,
        is_timed: true,
        description: None,
    },
    Exercise {
        id: "squats_strikes",
        name: "приседания с ударами",
        category: Category::Legs,
        muscle_groups: &[MuscleGroup::Quads, MuscleGroup::Glutes, MuscleGroup::Core, MuscleGroup::Shoulders],
        is_base: true,
        is_timed: false,
        description: None,
    },
    Exercise {
        id: "taiji_shadow",
        name: "тайцзи бой с тенью",
        category: Category::Taiji,
        muscle_groups: &[MuscleGroup::FullBody],
        is_base: true,
        is_timed: true,
        description: None,
    },
];

/// Дополнительные упражнения (из книги "You Are Your Own Gym")
pub const EXTRA_EXERCISES: &[Exercise] = &[
    // Тяговые (спина, бицепс)
    Exercise {
        id: "let_me_in",
        name: "впусти меня",
        category: Category::Pull,
        muscle_groups: &[MuscleGroup::Back, MuscleGroup::Biceps, MuscleGroup::Shoulders],
        is_base: false,
        is_timed: false,
        description: Some("Стоя лицом к двери, держась за ручки с двух сторон. Ноги по бокам двери. Подтягивайся к двери, сгибая локти"),
    },
    Exercise {
        id: "shelf_pullup",
        name: "подтягивание у полки",
        category: Category::Pull,
        muscle_groups: &[MuscleGroup::Biceps, MuscleGroup::Back],
        is_base: false,
        is_timed: false,
        description: Some("Встань у полки/перил на уровне пояса. Руки ладонями вверх под выступ. Тяни вверх, наклоняясь вперёд"),
    },
    // Ноги
    Exercise {
        id: "calf_raises",
        name: "подъём на носки",
        category: Category::Legs,
        muscle_groups: &[MuscleGroup::Calves],
        is_base: false,
        is_timed: false,
        description: Some("Встань на край ступеньки носками. Поднимайся на носки и опускайся ниже уровня ступени"),
    },
    Exercise {
        id: "romanian_deadlift",
        name: "румынская тяга на одной ноге",
        category: Category::Legs,
        muscle_groups: &[MuscleGroup::Hamstrings, MuscleGroup::Glutes, MuscleGroup::Core],
        is_base: false,
        is_timed: false,
        description: Some("Стоя на одной ноге, наклоняйся вперёд, отводя другую ногу назад. Спина прямая"),
    },
];

pub fn get_base_exercises() -> &'static [Exercise] {
    BASE_EXERCISES
}

pub fn get_all_exercises() -> Vec<&'static Exercise> {
    BASE_EXERCISES.iter().chain(EXTRA_EXERCISES.iter()).collect()
}

pub fn find_exercise(id: &str) -> Option<&'static Exercise> {
    get_all_exercises().into_iter().find(|e| e.id == id)
}

/// Find exercise by name (for matching DB records)
pub fn find_exercise_by_name(name: &str) -> Option<&'static Exercise> {
    get_all_exercises().into_iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_muscle_group_name_ru_all_groups() {
        // Проверяем, что все группы мышц имеют русские названия
        assert_eq!(MuscleGroup::Chest.name_ru(), "грудные");
        assert_eq!(MuscleGroup::Shoulders.name_ru(), "плечи");
        assert_eq!(MuscleGroup::Triceps.name_ru(), "трицепс");
        assert_eq!(MuscleGroup::Back.name_ru(), "спина");
        assert_eq!(MuscleGroup::Biceps.name_ru(), "бицепс");
        assert_eq!(MuscleGroup::Core.name_ru(), "кор");
        assert_eq!(MuscleGroup::Glutes.name_ru(), "ягодицы");
        assert_eq!(MuscleGroup::Quads.name_ru(), "квадрицепсы");
        assert_eq!(MuscleGroup::Hamstrings.name_ru(), "бицепс бедра");
        assert_eq!(MuscleGroup::Calves.name_ru(), "икры");
        assert_eq!(MuscleGroup::FullBody.name_ru(), "всё тело");
    }

    #[test]
    fn test_muscle_group_all_returns_11_groups() {
        let groups = MuscleGroup::all();
        assert_eq!(groups.len(), 11);
    }

    #[test]
    fn test_muscle_group_all_no_duplicates() {
        let groups = MuscleGroup::all();
        let mut seen = std::collections::HashSet::new();
        for g in groups {
            assert!(seen.insert(g), "Duplicate muscle group: {:?}", g);
        }
    }

    #[test]
    fn test_category_emoji_all_categories() {
        assert!(!Category::Push.emoji().is_empty());
        assert!(!Category::Pull.emoji().is_empty());
        assert!(!Category::Core.emoji().is_empty());
        assert!(!Category::Legs.emoji().is_empty());
        assert!(!Category::Taiji.emoji().is_empty());
        assert!(!Category::Strikes.emoji().is_empty());
    }

    #[test]
    fn test_get_base_exercises_count() {
        let exercises = get_base_exercises();
        assert_eq!(exercises.len(), 6);
    }

    #[test]
    fn test_get_all_exercises_count() {
        let exercises = get_all_exercises();
        // 6 базовых + 4 дополнительных = 10
        assert_eq!(exercises.len(), 10);
    }

    #[test]
    fn test_find_exercise_by_id_found() {
        let ex = find_exercise("pushups_fist");
        assert!(ex.is_some());
        assert_eq!(ex.unwrap().name, "отжимания на кулаках");
    }

    #[test]
    fn test_find_exercise_by_id_not_found() {
        let ex = find_exercise("nonexistent_exercise");
        assert!(ex.is_none());
    }

    #[test]
    fn test_find_exercise_by_name_found() {
        let ex = find_exercise_by_name("стойка на локтях");
        assert!(ex.is_some());
        assert_eq!(ex.unwrap().id, "plank_elbows");
    }

    #[test]
    fn test_find_exercise_by_name_not_found() {
        let ex = find_exercise_by_name("несуществующее упражнение");
        assert!(ex.is_none());
    }

    #[test]
    fn test_base_exercises_have_is_base_true() {
        for ex in get_base_exercises() {
            assert!(ex.is_base, "Base exercise {} should have is_base=true", ex.id);
        }
    }

    #[test]
    fn test_extra_exercises_have_is_base_false() {
        for ex in EXTRA_EXERCISES {
            assert!(!ex.is_base, "Extra exercise {} should have is_base=false", ex.id);
        }
    }

    #[test]
    fn test_timed_exercises() {
        // plank_elbows и taiji_shadow должны быть is_timed=true
        let plank = find_exercise("plank_elbows").unwrap();
        assert!(plank.is_timed, "Plank should be timed exercise");

        let taiji = find_exercise("taiji_shadow").unwrap();
        assert!(taiji.is_timed, "Taiji should be timed exercise");

        // Отжимания не на время
        let pushups = find_exercise("pushups_fist").unwrap();
        assert!(!pushups.is_timed, "Pushups should not be timed exercise");
    }

    #[test]
    fn test_all_exercises_have_muscle_groups() {
        for ex in get_all_exercises() {
            assert!(!ex.muscle_groups.is_empty(),
                "Exercise {} should have at least one muscle group", ex.id);
        }
    }

    #[test]
    fn test_all_exercises_have_unique_ids() {
        let exercises = get_all_exercises();
        let mut seen = std::collections::HashSet::new();
        for ex in exercises {
            assert!(seen.insert(ex.id), "Duplicate exercise ID: {}", ex.id);
        }
    }

    #[test]
    fn test_extra_exercises_have_descriptions() {
        for ex in EXTRA_EXERCISES {
            assert!(ex.description.is_some(),
                "Extra exercise {} should have description", ex.id);
        }
    }
}
