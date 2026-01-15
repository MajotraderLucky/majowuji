# majowuji Dashboard

## Project Status

| Component           | Status       | Progress |
|---------------------|--------------|----------|
| Telegram Bot        | [+] Deployed | 100%     |
| SQLite Storage      | [+] Ready    | 100%     |
| CLI                 | [+] Ready    | 100%     |
| Hourly Reminders    | [+] Working  | 100%     |
| Duration Tracking   | [+] Working  | 100%     |
| Pulse Tracking      | [+] Working  | 100%     |
| ML Recommendations  | [+] Working  | 100%     |
| Book Exercises      | [+] Working  | 100%     |
| Muscle Balance      | [+] Working  | 100%     |
| Multi-user Support  | [+] Working  | 100%     |
| Command Buttons     | [+] Working  | 100%     |

## Training Program

### Core Exercises (Martial Arts)

| Category         | Exercises                                     |
|------------------|-----------------------------------------------|
| Punches          | jab, cross, hook, uppercut                    |
| Kicks            | roundhouse, front-kick, side-kick, low-kick   |
| Taijiquan        | taiji-form, silk-reeling, push-hands          |
| Combinations     | shadow-boxing, bag-work, pad-work             |

### Supplementary (from "You Are Your Own Gym")

Book location: `docs/you-are-your-own-gym.txt`

| Chapter | Topic                    | Status      |
|---------|--------------------------|-------------|
| 1       | Intro & Philosophy       | [ ] To read |
| 2       | Author's Journey         | [ ] To read |
| 3       | Bodyweight Fundamentals  | [ ] To read |
| 4       | Push exercises           | [ ] To read |
| 5       | Pull exercises           | [ ] To read |
| 6       | Core exercises           | [ ] To read |
| 7       | Legs exercises           | [ ] To read |
| 8       | Training programs        | [ ] To read |

## Development Roadmap

### Phase 1: Foundation [DONE]
- [+] Project scaffolding
- [+] CLI with basic commands
- [+] SQLite database
- [+] Telegram bot deployed

### Phase 2: Analytics [DONE]
- [+] Duration tracking per exercise
- [+] Pulse tracking (before/after)
- [+] Exercise volume tracking
- [+] Inline command buttons

### Phase 3: Intelligence [DONE]
- [+] Muscle group tracking (11 groups)
- [+] Exercise recommendations by balance
- [+] /balance command (weekly report)
- [+] Book exercises with descriptions

### Phase 4: Integration [DONE]
- [+] Telegram bot with DB
- [+] Hourly reminders (systemd on archbook)
- [+] Multi-user support (10 users)

See [docs/DEPLOY.md](docs/DEPLOY.md) for deployment instructions.

## Recent Changes (2026-01-15)

| Change                                     | Status      |
|--------------------------------------------|-------------|
| 7-day record consolidation period          | [+] Done    |
| Show days remaining in consolidation       | [+] Done    |
| Auto-extend if record not confirmed        | [+] Done    |

## Changes (2026-01-14)

| Change                                     | Status      |
|--------------------------------------------|-------------|
| Fix clippy warnings (10 warnings)          | [+] Done    |
| Include Cargo.lock for reproducible builds | [+] Done    |
| Fix "NEW RECORD" bug on repeated result    | [+] Done    |
| Show both goals: simple +1 and ML target   | [+] Done    |
| Add average metrics (7/14 days)            | [+] Done    |
| Restructure base program (8 exercises)     | [+] Done    |
| Add taiji_shadow_weapon (cooldown)         | [+] Done    |
| Add swimmer to base program                | [+] Done    |
| Change beat record target to +1 for timed  | [+] Done    |
| Add base program completion summary        | [+] Done    |

**Base Program Summary:**
After completing the last base exercise, shows:
- Full list of 8 exercises with results
- New records highlighted with trophy
- Total time and sets count
- Today's muscle balance
- Smooth transition to bonus recommendation

**Goal Display Logic:**
- Consolidation: "Рекорд: 23 (закрепляем, 5 дн.)" - shown with days remaining
- Challenge: "Рекорд: 23 → побей: 24" - shown after record confirmed in 7-day window
- Auto-extend: If record not reached in 7 days, consolidation extends another 7 days
- ML: "~20 (усталость грудные)" - shown when differs from simple +1

## Backlog

| Task                           | Priority |
|--------------------------------|----------|
| Add error logging to bot       | Low      |
| Weekly/monthly summary reports | Low      |

## Telegram Commands

```
/train   - выбрать упражнение (с рекомендацией)
/today   - сегодняшние тренировки
/stats   - статистика
/balance - баланс нагрузки по группам мышц
/tip     - совет из книги
/remind  - напоминания раз в час
/stop    - выключить напоминания
```

После каждого сообщения бота доступны inline-кнопки для быстрого доступа к командам.

## Notes

- Database: `majowuji.db` (SQLite, auto-created)
- Telegram token: `TELOXIDE_TOKEN` env var
- Max users: `MAX_USERS` env var (default: 10)

## ML Data Collection

Bot collects the following data for ML analysis:

| Field         | Description                      | ML Use Case                    |
|---------------|----------------------------------|--------------------------------|
| date          | Timestamp (MSK)                  | Time-of-day performance        |
| duration_secs | Exercise duration                | Fatigue patterns               |
| pulse_before  | Heart rate before exercise       | Readiness indicator            |
| pulse_after   | Heart rate after exercise        | Recovery analysis              |
| reps          | Repetitions count                | Volume tracking                |
| exercise      | Exercise type                    | Category-based analysis        |

## Muscle Groups (11)

Each exercise is mapped to muscle groups for balance tracking:

| Group      | Description      | Exercises                                                |
|------------|------------------|----------------------------------------------------------|
| chest      | Грудные          | pushups_fist, pushups_handles                            |
| shoulders  | Плечи            | pushups, plank, squats_strikes                           |
| triceps    | Трицепс          | pushups_fist, pushups_handles                            |
| back       | Спина            | let_me_in, shelf_pullup                                  |
| biceps     | Бицепс           | let_me_in, shelf_pullup                                  |
| core       | Кор              | jackknife, plank, squats, pushups                        |
| quads      | Квадрицепсы      | squats_strikes                                           |
| glutes     | Ягодицы          | squats_strikes, romanian_deadlift                        |
| hamstrings | Бицепс бедра     | romanian_deadlift                                        |
| calves     | Икры             | calf_raises                                              |
| full_body  | Всё тело         | taiji_shadow, taiji_shadow_weapon, form_24, silk_reeling |

## Book Exercises System

Exercises from "Сам себе тренер" (You Are Your Own Gym) are offered as bonus after completing the base program for the day.

**Base Program (8 exercises with fixed order):**

| #   | Exercise             | Name                           | Role       |
|-----|----------------------|--------------------------------|------------|
| 1   | taiji_shadow         | тайцзи бой с тенью             | warmup     |
| 2   | pushups_fist         | отжимания на кулаках           | middle     |
| 3   | pushups_handles      | отжимания с ручками            | middle     |
| 4   | jackknife            | пресс складной нож             | middle     |
| 5   | plank_elbows         | стойка на локтях               | middle     |
| 6   | squats_strikes       | приседания с ударами           | middle     |
| 7   | swimmer              | пловец                         | middle     |
| 8   | taiji_shadow_weapon  | тайцзи бой с тенью с оружием   | cooldown   |

**Bonus Exercises (from book):**

| Exercise           | Muscle Groups             | Description                                    |
|--------------------|---------------------------|------------------------------------------------|
| form_24            | full_body                 | Классическая форма тайцзицюань из 24 движений  |
| silk_reeling       | full_body, core           | Упражнение на спиральную силу из стиля Чэнь    |
| let_me_in          | back, biceps, shoulders   | Подтягивания к двери, держась за ручки         |
| shelf_pullup       | biceps, back              | Тяга к полке/перилам ладонями вверх            |
| calf_raises        | calves                    | Подъём на носки на краю ступеньки              |
| romanian_deadlift  | hamstrings, glutes, core  | Румынская тяга на одной ноге                   |

**How it works:**
1. Bot always recommends taiji_shadow first (warmup)
2. Then middle exercises sorted by muscle balance
3. taiji_shadow_weapon recommended last (cooldown)
4. After all 8 base exercises done, bot offers bonus with description

## Multi-user System

| Feature             | Description                                        |
|---------------------|----------------------------------------------------|
| User limit          | 10 free users (configurable via MAX_USERS env)     |
| Owner               | First user automatically becomes owner             |
| Data separation     | Each user sees only their own trainings/stats      |
| Access control      | New users after limit get prompt to message owner  |
| Message forwarding  | Messages from blocked users forwarded to owner     |
