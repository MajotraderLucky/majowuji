# majowuji 无极

Personal martial arts training tracker with ML predictions.

**无极 (wuji)** - "limitless", the state of infinite potential before yin and yang separate.

## Features

| Feature          | Status      | Description                          |
|------------------|-------------|--------------------------------------|
| Training Log     | [+] Ready   | Record exercises, sets, reps, notes  |
| TUI Dashboard    | [+] Ready   | Terminal UI with ratatui             |
| SQLite Storage   | [+] Ready   | Local database for all data          |
| Telegram Bot     | [~] Basic   | Remote logging via Telegram          |
| Analytics        | [~] Basic   | Volume tracking, frequency stats     |
| ML Predictions   | [ ] Planned | Training load recommendations        |

## Installation

```bash
# Clone the repository
git clone https://github.com/MajotraderLucky/majowuji.git
cd majowuji

# Build
cargo build --release

# Run
cargo run
```

## Usage

### TUI Dashboard

```bash
# Open terminal dashboard (default)
majowuji
# or
majowuji tui
```

Press `q` to quit, `r` to refresh.

### Log Training

```bash
# Basic logging
majowuji log jab -s 3 -r 50

# With notes
majowuji log roundhouse -s 5 -r 20 -n "Focus on hip rotation"

# Defaults: 1 set, 10 reps
majowuji log forms
```

### View History

```bash
# Last 10 trainings (default)
majowuji list

# Last 20 trainings
majowuji list -l 20
```

### Statistics

```bash
# Overall stats
majowuji stats

# Stats for specific exercise
majowuji stats jab
```

### Telegram Bot

```bash
# Start bot (set TELOXIDE_TOKEN env var or use --token)
export TELOXIDE_TOKEN="your_bot_token"
majowuji bot

# Or directly
majowuji bot --token "your_bot_token"
```

Bot commands:
- `/start` - Initialize bot
- `/help` - Show available commands
- `/log jab 3x50` - Log training
- `/today` - Show today's trainings
- `/stats` - Show statistics

## Tech Stack

| Component | Crate       | Purpose                    |
|-----------|-------------|----------------------------|
| TUI       | ratatui     | Terminal dashboard         |
| Telegram  | teloxide    | Bot for remote logging     |
| Database  | rusqlite    | Local SQLite storage       |
| CLI       | clap        | Command-line interface     |
| Async     | tokio       | Async runtime              |
| ML        | linfa       | Predictions (planned)      |

## Roadmap

- [ ] TUI: Add charts for progress visualization
- [ ] TUI: Interactive training input
- [ ] Bot: Database integration
- [ ] Bot: Daily reminders
- [ ] ML: Training load prediction
- [ ] ML: Recovery time estimation
- [ ] ML: Technique improvement suggestions
- [ ] Export: Training data to JSON/CSV

## License

MIT
