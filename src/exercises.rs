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
