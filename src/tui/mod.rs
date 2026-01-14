//! TUI module - Terminal dashboard with ratatui

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Table, Row, Cell},
};
use std::io::{stdout, Stdout};

use crate::db::{Database, Training};

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// App state for TUI
pub struct App {
    db: Database,
    trainings: Vec<Training>,
    should_quit: bool,
}

impl App {
    pub fn new(db: Database) -> Result<Self> {
        let trainings = db.get_trainings()?;
        Ok(Self {
            db,
            trainings,
            should_quit: false,
        })
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<()> {
        let mut terminal = init_terminal()?;

        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        restore_terminal()?;
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        // Header
        let header = Paragraph::new("无极 majowuji - Training Tracker")
            .style(Style::default().fg(Color::Cyan).bold())
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, chunks[0]);

        // Training table
        let rows: Vec<Row> = self.trainings.iter().map(|t| {
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string()),
                Cell::from(t.exercise.clone()),
                Cell::from(format!("{}x{}", t.sets, t.reps)),
                Cell::from(t.notes.clone().unwrap_or_default()),
            ])
        }).collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(20),
                Constraint::Length(10),
                Constraint::Min(20),
            ],
        )
        .header(Row::new(vec!["Date", "Exercise", "Sets x Reps", "Notes"])
            .style(Style::default().bold()))
        .block(Block::default().borders(Borders::ALL).title("Trainings"));

        frame.render_widget(table, chunks[1]);

        // Footer
        let footer = Paragraph::new("q: quit | a: add | r: refresh")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => self.should_quit = true,
                        KeyCode::Char('r') => {
                            self.trainings = self.db.get_trainings()?;
                        }
                        _ => {}
                    }
                }
        Ok(())
    }
}

fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
