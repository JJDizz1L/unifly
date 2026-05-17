//! Events screen — live event stream with pause/filter (spec §2.7).

use std::sync::Arc;

use chrono::{Datelike, Timelike};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use tokio::sync::mpsc::UnboundedSender;

use unifly_api::Event;
use unifly_api::model::EventSeverity;

use crate::tui::action::Action;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct EventsScreen {
    focused: bool,
    events: Vec<Arc<Event>>,
    paused: bool,
    scroll_offset: usize,
    /// Max events to keep in memory.
    capacity: usize,
}

impl Default for EventsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl EventsScreen {
    pub fn new() -> Self {
        Self {
            focused: false,
            events: Vec::new(),
            paused: false,
            scroll_offset: 0,
            capacity: 10_000,
        }
    }

    #[allow(dead_code, clippy::unused_self)]
    fn visible_count(&self, area_height: u16) -> usize {
        usize::from(area_height.saturating_sub(1))
    }
}

impl Component for EventsScreen {
    fn init(&mut self, _action_tx: UnboundedSender<Action>) -> Result<()> {
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                if !self.paused {
                    // Resume: snap to bottom
                    self.scroll_offset = 0;
                }
                Ok(Some(Action::ToggleEventPause))
            }
            KeyCode::Char('j') | KeyCode::Down if self.paused => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                Ok(None)
            }
            KeyCode::Char('k') | KeyCode::Up if self.paused => {
                self.scroll_offset =
                    (self.scroll_offset + 1).min(self.events.len().saturating_sub(1));
                Ok(None)
            }
            KeyCode::Char('g') if self.paused => {
                self.scroll_offset = self.events.len().saturating_sub(1);
                Ok(Some(Action::ScrollToTop))
            }
            KeyCode::Char('G') if self.paused => {
                self.scroll_offset = 0;
                Ok(Some(Action::ScrollToBottom))
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) && self.paused => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                Ok(Some(Action::PageDown))
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) && self.paused => {
                self.scroll_offset =
                    (self.scroll_offset + 10).min(self.events.len().saturating_sub(1));
                Ok(Some(Action::PageUp))
            }
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: &Action) -> Result<Option<Action>> {
        if let Action::EventReceived(event) = action {
            self.events.push(Arc::clone(event));
            if self.events.len() > self.capacity {
                self.events.remove(0);
            }
        }
        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let count = self.events.len();
        let title = format!(" Events ({count}) ");
        let block = Block::default()
            .title(title)
            .title_style(theme::title_style())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if self.focused {
                theme::border_focused()
            } else {
                theme::border_default()
            });

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let show_punchcard = inner.height >= 15 && inner.width >= 48;
        let layout = if show_punchcard {
            Layout::vertical([
                Constraint::Length(1), // status line
                Constraint::Length(8), // punchcard
                Constraint::Min(1),    // events
                Constraint::Length(1), // hints
            ])
            .split(inner)
        } else {
            Layout::vertical([
                Constraint::Length(1), // status line
                Constraint::Min(1),    // events
                Constraint::Length(1), // hints
            ])
            .split(inner)
        };

        frame.render_widget(Paragraph::new(self.status_line()), layout[0]);

        let (events_area, hints_area) = if show_punchcard {
            self.render_punchcard(frame, layout[1]);
            (layout[2], layout[3])
        } else {
            (layout[1], layout[2])
        };

        self.render_event_list(frame, events_area);
        Self::render_hints(frame, hints_area);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn id(&self) -> &'static str {
        "Events"
    }
}

impl EventsScreen {
    fn status_line(&self) -> Line<'static> {
        let live_indicator = if self.paused {
            Span::styled("PAUSED", Style::default().fg(theme::warning()))
        } else {
            Span::styled("● LIVE", Style::default().fg(theme::success()))
        };

        Line::from(vec![
            Span::styled("  Filter: ", Style::default().fg(theme::text_secondary())),
            Span::styled("[all]", Style::default().fg(theme::accent_secondary())),
            Span::styled("  Type: ", Style::default().fg(theme::text_secondary())),
            Span::styled("[all]", Style::default().fg(theme::accent_secondary())),
            Span::raw("  "),
            live_indicator,
        ])
    }

    fn render_event_list(&self, frame: &mut Frame, area: Rect) {
        let visible_height = usize::from(area.height);
        let end = self.events.len().saturating_sub(self.scroll_offset);
        let start = end.saturating_sub(visible_height);

        let mut lines = vec![Line::from(vec![
            Span::styled("  Time      ", theme::table_header()),
            Span::styled("Category   ", theme::table_header()),
            Span::styled("Message", theme::table_header()),
        ])];

        let meta_cols: u16 = 2 + 12 + 11; // indent + time + category
        let msg_width = usize::from(area.width.saturating_sub(meta_cols).max(10));

        for event in self.events.get(start..end).unwrap_or_default() {
            let time_str = event.timestamp.format("%H:%M:%S").to_string();
            let severity_color = match event.severity {
                EventSeverity::Error | EventSeverity::Critical => theme::error(),
                EventSeverity::Warning => theme::warning(),
                EventSeverity::Info => theme::accent_secondary(),
                _ => theme::text_secondary(),
            };
            let category = format!("{:?}", event.category);
            let msg: String = event.message.chars().take(msg_width).collect();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {time_str:<12}"),
                    Style::default().fg(theme::warning()),
                ),
                Span::styled(
                    format!("{category:<11}"),
                    Style::default().fg(theme::text_secondary()),
                ),
                Span::styled(msg, Style::default().fg(severity_color)),
            ]));
        }

        if self.events.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Waiting for events...",
                Style::default().fg(theme::border_unfocused()),
            )));
        }

        // Auto-scroll indicator
        if !self.paused && !self.events.is_empty() {
            lines.push(Line::from(Span::styled(
                "  ↓ auto-scrolling",
                Style::default().fg(theme::border_unfocused()),
            )));
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_hints(frame: &mut Frame, area: Rect) {
        let hints = Line::from(vec![
            Span::styled("  Space ", theme::key_hint_key()),
            Span::styled("pause/resume  ", theme::key_hint()),
            Span::styled("j/k ", theme::key_hint_key()),
            Span::styled("scroll (paused)  ", theme::key_hint()),
            Span::styled("/ ", theme::key_hint_key()),
            Span::styled("search", theme::key_hint()),
        ]);
        frame.render_widget(Paragraph::new(hints), area);
    }

    fn render_punchcard(&self, frame: &mut Frame, area: Rect) {
        let buckets = event_punchcard(&self.events);
        let max_count = buckets
            .iter()
            .flatten()
            .copied()
            .max()
            .unwrap_or_default()
            .max(1);
        let mut lines = vec![Line::from(vec![
            Span::styled("  Events by hour  ", theme::table_header()),
            Span::styled(
                "00      06      12      18",
                Style::default().fg(theme::text_muted()),
            ),
        ])];

        for (day, row) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .zip(buckets.iter())
        {
            let mut spans = vec![Span::styled(
                format!("  {day} "),
                Style::default().fg(theme::text_secondary()),
            )];
            for count in row {
                let (symbol, color) = punchcard_cell(*count, max_count);
                spans.push(Span::styled(symbol, Style::default().fg(color)));
            }
            lines.push(Line::from(spans));
        }

        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn event_punchcard(events: &[Arc<Event>]) -> [[usize; 24]; 7] {
    let mut buckets = [[0usize; 24]; 7];
    for event in events {
        let day = usize::try_from(event.timestamp.weekday().num_days_from_monday()).unwrap_or(0);
        let hour = usize::try_from(event.timestamp.hour()).unwrap_or(0);
        buckets[day][hour] = buckets[day][hour].saturating_add(1);
    }
    buckets
}

fn punchcard_cell(count: usize, max_count: usize) -> (&'static str, ratatui::style::Color) {
    if count == 0 {
        return ("░", theme::text_muted());
    }
    let colors = theme::event_density_colors();
    let max_count = max_count.max(1);
    let scaled = if max_count <= 1 || count.saturating_mul(3) <= max_count {
        1
    } else if count.saturating_mul(3) <= max_count.saturating_mul(2) {
        2
    } else {
        3
    };

    match scaled {
        1 => ("▒", colors[0]),
        2 => ("▓", colors[1]),
        _ => ("█", colors[2]),
    }
}
