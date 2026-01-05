# majowuji Dashboard

## Project Status

| Component           | Status       | Progress |
|---------------------|--------------|----------|
| CLI                 | [+] Ready    | 100%     |
| SQLite Storage      | [+] Ready    | 100%     |
| TUI Dashboard       | [~] Basic    | 40%      |
| Telegram Bot        | [+] Deployed | 100%     |
| Hourly Reminders    | [+] Working  | 100%     |
| Duration Tracking   | [+] Working  | 100%     |
| Pulse Tracking      | [+] Working  | 100%     |
| ML Recommendations  | [+] Working  | 80%      |
| Muscle Balance      | [+] Working  | 100%     |
| Multi-user Support  | [+] Working  | 100%     |
| Charts/Graphs       | [ ] Planned  | 0%       |

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
- [+] Basic TUI view
- [ ] Improve TUI with charts

### Phase 2: Analytics
- [+] Duration tracking per exercise
- [+] Pulse tracking (before/after)
- [ ] Progress graphs (sparklines)
- [ ] Weekly/monthly summaries
- [ ] Exercise volume tracking
- [ ] Personal records (PR) tracking

### Phase 3: Intelligence [CURRENT]
- [+] Muscle group tracking (11 groups)
- [+] Exercise recommendations by balance
- [+] /balance command (weekly report)
- [ ] ML-based load prediction
- [ ] Recovery recommendations based on pulse
- [ ] Pattern recognition

### Phase 4: Integration
- [+] Telegram bot with DB
- [+] Hourly reminders (systemd on archbook)
- [ ] Export to JSON/CSV
- [ ] Sync between devices

See [docs/DEPLOY.md](docs/DEPLOY.md) for deployment instructions.

## Current Sprint

| Task                                 | Status      |
|--------------------------------------|-------------|
| Test logging functionality           | [+] Done    |
| Add book to docs                     | [+] Done    |
| Create dashboard                     | [+] Done    |
| Deploy Telegram bot to archbook      | [+] Done    |
| Add hourly reminders                 | [+] Done    |
| Simplify input (just reps)           | [+] Done    |
| Add duration tracking                | [+] Done    |
| Add pulse tracking (HR before/after) | [+] Done    |
| Add muscle group tracking (11)       | [+] Done    |
| Add ML recommendations in /train     | [+] Done    |
| Add /balance command                 | [+] Done    |
| Multi-user support (10 users limit)  | [+] Done    |
| Add TUI progress charts              | [ ] Next    |
| ML load prediction based on pulse    | [ ] Backlog |

## Quick Commands

### CLI
```bash
# Log training
majowuji log jab -s 3 -r 50 -n "Notes here"

# View history
majowuji list

# Statistics
majowuji stats
majowuji stats jab

# TUI dashboard
majowuji tui
```

### Telegram Bot
```
/train   - выбрать упражнение (с рекомендацией)
/today   - сегодняшние тренировки
/stats   - статистика
/balance - баланс нагрузки по группам мышц
/remind  - напоминания раз в час
/stop    - выключить напоминания
```

## Notes

- Database file: `majowuji.db` (auto-created)
- Config: planned for `~/.config/majowuji/`
- Telegram token: set via `TELOXIDE_TOKEN` env var

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

| Group      | Description      | Exercises                           |
|------------|------------------|-------------------------------------|
| chest      | Грудные          | pushups_fist, pushups_handles       |
| shoulders  | Плечи            | pushups, plank, squats_strikes      |
| triceps    | Трицепс          | pushups_fist, pushups_handles       |
| core       | Кор              | jackknife, plank, squats, pushups   |
| quads      | Квадрицепсы      | squats_strikes                      |
| glutes     | Ягодицы          | squats_strikes                      |
| full_body  | Всё тело         | taiji_shadow, form_24, silk_reeling |

*Back, biceps, hamstrings, calves - planned with new exercises*

## Multi-user System

| Feature             | Description                                        |
|---------------------|----------------------------------------------------|
| User limit          | 10 free users (configurable via MAX_USERS env)     |
| Owner               | First user automatically becomes owner             |
| Data separation     | Each user sees only their own trainings/stats      |
| Access control      | New users after limit get prompt to message owner  |
| Message forwarding  | Messages from blocked users forwarded to owner     |

**Environment Variables:**
- `MAX_USERS` - Maximum allowed users (default: 10)
- `TELOXIDE_TOKEN` - Telegram bot token
