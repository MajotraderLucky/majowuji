//! Tips module - советы из книги "You Are Your Own Gym"

use rand::seq::SliceRandom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipCategory {
    Motivation,    // Мотивация
    Nutrition,     // Питание
    Training,      // Тренировки
    Technique,     // Техника упражнений
    Recovery,      // Восстановление
}

impl TipCategory {
    pub fn emoji(&self) -> &'static str {
        match self {
            TipCategory::Motivation => "💪",
            TipCategory::Nutrition => "🥗",
            TipCategory::Training => "🏋️",
            TipCategory::Technique => "📐",
            TipCategory::Recovery => "😴",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TipCategory::Motivation => "Мотивация",
            TipCategory::Nutrition => "Питание",
            TipCategory::Training => "Тренировка",
            TipCategory::Technique => "Техника",
            TipCategory::Recovery => "Восстановление",
        }
    }
}

pub struct Tip {
    pub category: TipCategory,
    pub text: &'static str,
}

/// Советы из книги "You Are Your Own Gym" Марка Лорена
pub const TIPS: &[Tip] = &[
    // === МОТИВАЦИЯ ===
    Tip {
        category: TipCategory::Motivation,
        text: "Единственное, что может вас остановить — это вы сами. Отбросьте всё, что мешает достичь цели.",
    },
    Tip {
        category: TipCategory::Motivation,
        text: "Лучший фитнес-тренажёр уже при вас — ваше собственное тело. И оно всегда с вами!",
    },
    Tip {
        category: TipCategory::Motivation,
        text: "Нет времени? Хорошие тренировки необязательно должны быть длинными. 20-30 минут 4 раза в неделю — достаточно.",
    },
    Tip {
        category: TipCategory::Motivation,
        text: "Успех спортивных тренировок непременно приведёт к успеху в других сферах жизни.",
    },
    Tip {
        category: TipCategory::Motivation,
        text: "Желание и усердие приводят к успеху. Чтобы придерживаться решения, надо расслабиться и держать форму.",
    },
    Tip {
        category: TipCategory::Motivation,
        text: "Напряжение, паника и беспокойство высасывают энергию. Оставайтесь расслабленным, чтобы пережить трудное.",
    },

    // === ПИТАНИЕ ===
    Tip {
        category: TipCategory::Nutrition,
        text: "Съедайте пищу в 5 приёмов за день, каждые 2,5-3,5 часа. Это поддержит уровень энергии стабильным.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "3 грамма белка на каждый килограмм вашего идеального веса — основа для сохранения и роста мышц.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Не морите себя голодом и не переедайте. Ешьте до того, как исчезнет чувство голода.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Телу нужно 15-20 минут, чтобы осознать, что голод утолён. Не торопитесь во время еды!",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Держитесь подальше от переработанных сахаров — они повсюду! Выбирайте углеводы с низким гликемическим индексом.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Пейте минимум 2 литра воды в день. Ваша моча должна быть бесцветной или слегка желтоватой.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Никогда не выходите из дома на голодный желудок. Съешьте что-нибудь заранее перед рестораном или вечеринкой.",
    },
    Tip {
        category: TipCategory::Nutrition,
        text: "Утром тело голодало всю ночь. Первый приём пищи запустит метаболизм и поступление питательных веществ.",
    },

    // === ТРЕНИРОВКИ (6 принципов) ===
    Tip {
        category: TipCategory::Training,
        text: "ПОСЛЕДОВАТЕЛЬНОСТЬ — настоящий страж длительного успеха. Не на пару месяцев, а на годы и десятки лет.",
    },
    Tip {
        category: TipCategory::Training,
        text: "ВОССТАНОВЛЕНИЕ: Содержится ли в программе время для отдыха? Переутомление — враг прогресса.",
    },
    Tip {
        category: TipCategory::Training,
        text: "РЕГУЛЯРНОСТЬ: Тело не приспособится к спонтанной активности. Ставьте цели и методично добивайтесь их.",
    },
    Tip {
        category: TipCategory::Training,
        text: "ВАРИАТИВНОСТЬ: Варьируйте интенсивность, объём и время отдыха. Не меняйте упражнения каждый день.",
    },
    Tip {
        category: TipCategory::Training,
        text: "ПРОГРЕСС: Не поднимайте одни и те же гантели годами. Переходите к более сложным вариациям упражнений.",
    },
    Tip {
        category: TipCategory::Training,
        text: "ПЕРЕГРУЗКА: Чтобы набрать силу, ставьте мышцы в неудобное положение. Телу нужен стимул для адаптации.",
    },

    // === ТЕХНИКА ===
    Tip {
        category: TipCategory::Technique,
        text: "Способы усложнить упражнение: повысить нагрузку, неустойчивая поверхность, паузы, движение одной конечностью.",
    },
    Tip {
        category: TipCategory::Technique,
        text: "Специально делайте паузу на 3 секунды в самой сложной части движения — это прекрасно вырабатывает силу.",
    },
    Tip {
        category: TipCategory::Technique,
        text: "После мышечного истощения попробуйте более лёгкую версию упражнения и доведите её до максимума.",
    },
    Tip {
        category: TipCategory::Technique,
        text: "Силовые упражнения задействуют сразу несколько групп мышц и сильно нагружают кор.",
    },
    Tip {
        category: TipCategory::Technique,
        text: "Чем ниже поверхность опоры при отжиманиях — тем тяжелее задача. Регулируйте сложность высотой.",
    },

    // === ВОССТАНОВЛЕНИЕ ===
    Tip {
        category: TipCategory::Recovery,
        text: "Силовая тренировка даёт импульс метаболизму на 48 часов. Вы сжигаете калории даже во сне!",
    },
    Tip {
        category: TipCategory::Recovery,
        text: "С возрастом тело теряет мышцы и метаболизм замедляется. Силовые тренировки восстанавливают юношеский метаболизм.",
    },
    Tip {
        category: TipCategory::Recovery,
        text: "Полкило мышц сжигает 10 калорий в день даже в покое. 2,5 кг мышц = минус 2,5 кг жира в год.",
    },
    Tip {
        category: TipCategory::Recovery,
        text: "Интервалы отдыха: 30-60 сек для выносливости, 90-120 сек для силы, 2,5-5 мин для мощности.",
    },
];

/// Получить случайный совет
pub fn get_random_tip() -> &'static Tip {
    TIPS.choose(&mut rand::thread_rng()).unwrap_or(&TIPS[0])
}

/// Получить случайный совет определённой категории
pub fn get_random_tip_by_category(category: TipCategory) -> Option<&'static Tip> {
    let filtered: Vec<_> = TIPS.iter().filter(|t| t.category == category).collect();
    filtered.choose(&mut rand::thread_rng()).copied()
}

/// Форматировать совет для отправки
pub fn format_tip(tip: &Tip) -> String {
    format!(
        "{} {}\n\n{}",
        tip.category.emoji(),
        tip.category.name(),
        tip.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tip_category_emoji_all_categories() {
        assert!(!TipCategory::Motivation.emoji().is_empty());
        assert!(!TipCategory::Nutrition.emoji().is_empty());
        assert!(!TipCategory::Training.emoji().is_empty());
        assert!(!TipCategory::Technique.emoji().is_empty());
        assert!(!TipCategory::Recovery.emoji().is_empty());
    }

    #[test]
    fn test_tip_category_name_all_categories() {
        assert_eq!(TipCategory::Motivation.name(), "Мотивация");
        assert_eq!(TipCategory::Nutrition.name(), "Питание");
        assert_eq!(TipCategory::Training.name(), "Тренировка");
        assert_eq!(TipCategory::Technique.name(), "Техника");
        assert_eq!(TipCategory::Recovery.name(), "Восстановление");
    }

    #[test]
    fn test_tips_not_empty() {
        assert!(!TIPS.is_empty());
        // Должно быть минимум 20 советов
        assert!(TIPS.len() >= 20, "Expected at least 20 tips, got {}", TIPS.len());
    }

    #[test]
    fn test_tips_count() {
        // 29 советов
        assert_eq!(TIPS.len(), 29);
    }

    #[test]
    fn test_get_random_tip_never_panics() {
        // Вызываем несколько раз, не должно паниковать
        for _ in 0..10 {
            let tip = get_random_tip();
            assert!(!tip.text.is_empty());
        }
    }

    #[test]
    fn test_get_random_tip_by_category_returns_correct_category() {
        // Проверяем каждую категорию
        for category in [
            TipCategory::Motivation,
            TipCategory::Nutrition,
            TipCategory::Training,
            TipCategory::Technique,
            TipCategory::Recovery,
        ] {
            let tip = get_random_tip_by_category(category);
            assert!(tip.is_some(), "Category {:?} should have tips", category);
            assert_eq!(tip.unwrap().category, category);
        }
    }

    #[test]
    fn test_format_tip_contains_emoji() {
        let tip = &TIPS[0];
        let formatted = format_tip(tip);
        assert!(formatted.contains(tip.category.emoji()));
    }

    #[test]
    fn test_format_tip_contains_category_name() {
        let tip = &TIPS[0];
        let formatted = format_tip(tip);
        assert!(formatted.contains(tip.category.name()));
    }

    #[test]
    fn test_format_tip_contains_text() {
        let tip = &TIPS[0];
        let formatted = format_tip(tip);
        assert!(formatted.contains(tip.text));
    }

    #[test]
    fn test_all_tips_have_non_empty_text() {
        for (i, tip) in TIPS.iter().enumerate() {
            assert!(!tip.text.is_empty(), "Tip {} has empty text", i);
        }
    }

    #[test]
    fn test_tips_distribution_by_category() {
        // Проверяем, что в каждой категории есть советы
        let mut counts = std::collections::HashMap::new();
        for tip in TIPS.iter() {
            *counts.entry(tip.category).or_insert(0) += 1;
        }

        assert!(counts.get(&TipCategory::Motivation).unwrap_or(&0) >= &3,
            "Motivation should have at least 3 tips");
        assert!(counts.get(&TipCategory::Nutrition).unwrap_or(&0) >= &3,
            "Nutrition should have at least 3 tips");
        assert!(counts.get(&TipCategory::Training).unwrap_or(&0) >= &3,
            "Training should have at least 3 tips");
        assert!(counts.get(&TipCategory::Technique).unwrap_or(&0) >= &3,
            "Technique should have at least 3 tips");
        assert!(counts.get(&TipCategory::Recovery).unwrap_or(&0) >= &3,
            "Recovery should have at least 3 tips");
    }
}
