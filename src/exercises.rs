//! Exercise definitions - база упражнений

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub is_base: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Category {
    Push,      // Отжимания, жимы
    Core,      // Пресс, планка
    Legs,      // Ноги, приседания
    Taiji,     // Тайцзицюань
    Strikes,   // Удары
}

impl Category {
    pub fn emoji(&self) -> &'static str {
        match self {
            Category::Push => "💪",
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
        is_base: true,
    },
    Exercise {
        id: "pushups_handles",
        name: "отжимания с ручками",
        category: Category::Push,
        is_base: true,
    },
    Exercise {
        id: "jackknife",
        name: "пресс складной нож",
        category: Category::Core,
        is_base: true,
    },
    Exercise {
        id: "plank_elbows",
        name: "стойка на локтях",
        category: Category::Core,
        is_base: true,
    },
    Exercise {
        id: "squats_strikes",
        name: "приседания с ударами",
        category: Category::Legs,
        is_base: true,
    },
    Exercise {
        id: "taiji_shadow",
        name: "тайцзи бой с тенью",
        category: Category::Taiji,
        is_base: true,
    },
];

/// Дополнительные упражнения (из книги)
pub const EXTRA_EXERCISES: &[Exercise] = &[
    // Будут добавляться по мере изучения книги
    Exercise {
        id: "form_24",
        name: "форма 24",
        category: Category::Taiji,
        is_base: false,
    },
    Exercise {
        id: "silk_reeling",
        name: "чаньсыгун",
        category: Category::Taiji,
        is_base: false,
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
