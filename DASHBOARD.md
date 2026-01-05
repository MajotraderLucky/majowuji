# majowuji Dashboard

## Project Status

| Component        | Status       | Progress |
|------------------|--------------|----------|
| CLI              | [+] Ready    | 100%     |
| SQLite Storage   | [+] Ready    | 100%     |
| TUI Dashboard    | [~] Basic    | 40%      |
| Telegram Bot     | [+] Deployed | 80%      |
| Hourly Reminders | [+] Working  | 100%     |
| ML Analytics     | [~] Basic    | 20%      |
| Charts/Graphs    | [ ] Planned  | 0%       |

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

### Phase 1: Foundation [CURRENT]
- [+] Project scaffolding
- [+] CLI with basic commands
- [+] SQLite database
- [+] Basic TUI view
- [ ] Improve TUI with charts

### Phase 2: Analytics
- [ ] Progress graphs (sparklines)
- [ ] Weekly/monthly summaries
- [ ] Exercise volume tracking
- [ ] Personal records (PR) tracking

### Phase 3: Intelligence
- [ ] ML-based load prediction
- [ ] Recovery recommendations
- [ ] Optimal training suggestions
- [ ] Pattern recognition

### Phase 4: Integration
- [+] Telegram bot with DB
- [+] Hourly reminders (systemd on archbook)
- [ ] Export to JSON/CSV
- [ ] Sync between devices

See [docs/DEPLOY.md](docs/DEPLOY.md) for deployment instructions.

## Current Sprint

| Task                               | Status      |
|------------------------------------|-------------|
| Test logging functionality         | [+] Done    |
| Add book to docs                   | [+] Done    |
| Create dashboard                   | [+] Done    |
| Deploy Telegram bot to archbook    | [+] Done    |
| Add hourly reminders               | [+] Done    |
| Simplify input (just reps)         | [~] In dev  |
| Add TUI progress charts            | [ ] Next    |
| Implement exercise categories      | [ ] Backlog |

## Quick Commands

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

## Notes

- Database file: `majowuji.db` (auto-created)
- Config: planned for `~/.config/majowuji/`
- Telegram token: set via `TELOXIDE_TOKEN` env var
