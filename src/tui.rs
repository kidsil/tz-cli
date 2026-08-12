//! Interactive world clock: a navigable instant shown across several zones,
//! with a month calendar. Bare `tz` (or `-i`) launches this.
//!
//! Keys:
//!   ↑/↓  k/j        select city      ←/→  h/l   ± 1 hour
//!   [ / ]           ± 15 minutes
//!   Ctrl+←/→        ± 1 day          Ctrl+↑/↓  ± 1 week
//!   PgUp / PgDn     ± 1 month
//!   t / n / 0       jump to now      q / Esc   quit

use std::time::Duration;

use jiff::{ToSpan, Zoned};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::cities::Resolved;

/// Gentle dusty-rose used to mark weekends (Sat/Sun).
const WEEKEND: Color = Color::Rgb(0xCB, 0x8A, 0x8A);

struct App {
    zones: Vec<Resolved>,
    recent: Vec<(Resolved, u32)>, // recently/frequently checked, with counts
    anchor_idx: usize,
    cursor: Zoned, // expressed in zones[anchor_idx].tz
    use_24h: bool,
}

impl App {
    fn new(zones: Vec<Resolved>, recent: Vec<(Resolved, u32)>, use_24h: bool) -> Self {
        let cursor = jiff::Timestamp::now().to_zoned(zones[0].tz.clone());
        App {
            zones,
            recent,
            anchor_idx: 0,
            cursor,
            use_24h,
        }
    }

    fn shift(&mut self, span: jiff::Span) {
        if let Ok(z) = self.cursor.checked_add(span) {
            self.cursor = z;
        }
    }

    /// Shift by whole months, clamping the day to the target month length and
    /// re-resolving the wall-clock time in the anchor zone (DST-safe).
    fn shift_months(&mut self, n: i64) {
        let d = self.cursor.date();
        let total = d.year() as i64 * 12 + (d.month() as i64 - 1) + n;
        let year = total.div_euclid(12) as i16;
        let month = (total.rem_euclid(12) + 1) as i8;
        let day = d.day().min(days_in_month(year, month));
        if let Ok(nd) = jiff::civil::Date::new(year, month, day) {
            let dt = nd.to_datetime(self.cursor.time());
            if let Ok(z) = self.zones[self.anchor_idx].tz.to_zoned(dt) {
                self.cursor = z;
            }
        }
    }

    fn now(&mut self) {
        self.cursor = jiff::Timestamp::now().to_zoned(self.zones[self.anchor_idx].tz.clone());
    }

    /// Move the selection (anchor) between cities, keeping the same instant.
    fn select_city(&mut self, delta: isize) {
        let n = self.zones.len() as isize;
        if n == 0 {
            return;
        }
        let instant = self.cursor.timestamp();
        self.anchor_idx = (self.anchor_idx as isize + delta).rem_euclid(n) as usize;
        self.cursor = instant.to_zoned(self.zones[self.anchor_idx].tz.clone());
    }

    fn time_fmt(&self) -> &'static str {
        if self.use_24h {
            "%H:%M"
        } else {
            "%-I:%M %p"
        }
    }
}

pub fn run(
    zones: Vec<Resolved>,
    recent: Vec<(Resolved, u32)>,
    use_24h: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut app = App::new(zones, recent, use_24h);
    let res = loop {
        if let Err(e) = terminal.draw(|f| draw(f, &app)) {
            break Err(e.into());
        }
        // Poll so the "now" line stays live even without input.
        match event::poll(Duration::from_millis(500)) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => break Err(e.into()),
        }
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(e) => break Err(e.into()),
        };
        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Char('c') if ctrl => break Ok(()),

                // Date navigation: Ctrl + arrows (left/right = day, up/down = week).
                KeyCode::Left if ctrl => app.shift((-1i64).days()),
                KeyCode::Right if ctrl => app.shift(1i64.days()),
                KeyCode::Up if ctrl => app.shift((-7i64).days()),
                KeyCode::Down if ctrl => app.shift(7i64.days()),

                // Select between cities: plain Up/Down (or k/j).
                KeyCode::Up | KeyCode::Char('k') => app.select_city(-1),
                KeyCode::Down | KeyCode::Char('j') => app.select_city(1),

                // Nudge the time: plain Left/Right (or h/l), [ ] for 15 min.
                KeyCode::Left | KeyCode::Char('h') => app.shift((-60i64).minutes()),
                KeyCode::Right | KeyCode::Char('l') => app.shift(60i64.minutes()),
                KeyCode::Char('[') => app.shift((-15i64).minutes()),
                KeyCode::Char(']') => app.shift(15i64.minutes()),

                // Bigger jumps.
                KeyCode::PageUp => app.shift_months(-1),
                KeyCode::PageDown => app.shift_months(1),

                KeyCode::Tab => app.select_city(1),
                KeyCode::Char('t') | KeyCode::Char('n') | KeyCode::Char('0') => app.now(),
                _ => {}
            }
        }
    };
    ratatui::restore();
    res
}

fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::vertical([
        Constraint::Length(3), // header
        Constraint::Min(8),    // body
        Constraint::Length(1), // footer
    ])
    .split(f.area());

    draw_header(f, outer[0], app);

    let body = Layout::horizontal([Constraint::Min(30), Constraint::Length(26)]).split(outer[1]);
    draw_zones(f, body[0], app);
    draw_calendar(f, body[1], app);

    draw_footer(f, outer[2]);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let anchor = &app.zones[app.anchor_idx];
    let head = format!(
        " {} · {}   ",
        app.cursor.strftime("%A, %b %-d %Y"),
        app.cursor.strftime(app.time_fmt()),
    );
    let line = Line::from(vec![
        Span::styled(
            head,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(anchor.display.clone(), Style::default().fg(Color::DarkGray)),
    ]);
    let block = Block::default().borders(Borders::ALL).title(" tz ");
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_zones(f: &mut Frame, area: Rect, app: &App) {
    let instant = app.cursor.timestamp();
    let ref_date = instant.to_zoned(app.zones[app.anchor_idx].tz.clone()).date();

    // Align city column across both the main zones and the recent list.
    let w_city = app
        .zones
        .iter()
        .map(|z| z.display.len())
        .chain(app.recent.iter().map(|(z, _)| z.display.len()))
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();
    for (i, z) in app.zones.iter().enumerate() {
        let is_anchor = i == app.anchor_idx;
        lines.push(zone_row(z, instant, ref_date, w_city, is_anchor, None, app));
    }

    // Separator + recently/frequently checked cities.
    if !app.recent.is_empty() {
        lines.push(Line::from(Span::styled(
            "── recent ────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        for (z, count) in &app.recent {
            lines.push(zone_row(z, instant, ref_date, w_city, false, Some(*count), app));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" zones (↑/↓ select) ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// Render a single zone row. `count = Some(n)` marks it as a (dimmer) recent
/// entry showing how many times it's been checked; `None` is a main zone.
fn zone_row<'a>(
    z: &'a Resolved,
    instant: jiff::Timestamp,
    ref_date: jiff::civil::Date,
    w_city: usize,
    is_anchor: bool,
    count: Option<u32>,
    app: &App,
) -> Line<'a> {
    let zoned = instant.to_zoned(z.tz.clone());
    let diff = zoned.date().since(ref_date).map(|s| s.get_days()).unwrap_or(0);
    let delta = match diff {
        0 => String::new(),
        1 => "  (next day)".into(),
        -1 => "  (prev day)".into(),
        n if n > 0 => format!("  (+{n}d)"),
        n => format!("  ({n}d)"),
    };
    let time = zoned.strftime(app.time_fmt()).to_string();
    // One labelling convention for the whole table, not per row: across a New
    // Year boundary some zones are already in January while others are still in
    // December, and labelling only the odd ones out is what creates the
    // ambiguity the year is there to remove.
    let date_str = {
        let anchor_year = app.cursor.year();
        let mixed = app
            .zones
            .iter()
            .any(|z| app.cursor.timestamp().to_zoned(z.tz.clone()).year() != anchor_year);
        if mixed {
            zoned.strftime("%a %b %-d %Y").to_string()
        } else {
            zoned.strftime("%a %b %-d").to_string()
        }
    };
    let tail = format!("  {}  {}", zoned.strftime("%Z"), offset(&zoned));
    let is_weekend = zoned.date().weekday().to_monday_zero_offset() >= 5;
    let date_style = if is_weekend {
        Style::default().fg(WEEKEND)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let city = format!("{:<width$}", z.display, width = w_city);

    let is_recent = count.is_some();
    let marker = if is_anchor { "▶ " } else { "  " };
    let time_style = if is_anchor {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if is_recent {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    let city_style = if is_recent {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::Green)
    };
    let freq = match count {
        Some(n) => format!("  ×{n}"),
        None => String::new(),
    };

    Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Cyan)),
        Span::styled(format!("{time:>8}  "), time_style),
        Span::styled(city, city_style),
        Span::raw("   "),
        Span::styled(date_str, date_style),
        Span::styled(tail, Style::default().fg(Color::DarkGray)),
        Span::styled(delta, Style::default().fg(Color::Yellow)),
        Span::styled(freq, Style::default().fg(Color::DarkGray)),
    ])
}

fn draw_calendar(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.cursor.date();
    let today = jiff::Timestamp::now()
        .to_zoned(app.zones[app.anchor_idx].tz.clone())
        .date();

    let first = jiff::civil::Date::new(cur.year(), cur.month(), 1).unwrap_or(cur);
    // Monday = 0 .. Sunday = 6
    let lead = first.weekday().to_monday_zero_offset() as usize;
    let days_in_month = days_in_month(cur.year(), cur.month());

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        cur.strftime("     %B %Y").to_string(),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled(" Mo Tu We Th Fr ", Style::default().fg(Color::DarkGray)),
        Span::styled("Sa Su", Style::default().fg(WEEKEND)),
    ]));

    let mut spans: Vec<Span> = vec![Span::raw(" ".repeat(1 + lead * 3))];
    let mut col = lead;
    for day in 1..=days_in_month {
        let label = format!("{day:>2} ");
        let is_today =
            day == today.day() && cur.year() == today.year() && cur.month() == today.month();
        let style = if day == cur.day() {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else if is_today {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if col >= 5 {
            // Saturday (col 5) or Sunday (col 6).
            Style::default().fg(WEEKEND)
        } else {
            Style::default()
        };
        spans.push(Span::styled(label, style));
        col += 1;
        if col == 7 {
            lines.push(Line::from(std::mem::take(&mut spans)));
            spans.push(Span::raw(" "));
            col = 0;
        }
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    let block = Block::default().borders(Borders::ALL).title(" calendar ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let help = "  ↑/↓ city   ←/→ ±1h   [/] ±15m   ^←/^→ ±1d   ^↑/^↓ ±1wk   PgUp/Dn ±1mo   t now   q quit";
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            help,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

fn offset(z: &Zoned) -> String {
    let secs = z.offset().seconds();
    let sign = if secs < 0 { '-' } else { '+' };
    let secs = secs.abs();
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

fn days_in_month(year: i16, month: i8) -> i8 {
    // First day of next month minus one day.
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    match jiff::civil::Date::new(ny, nm, 1) {
        Ok(first_next) => first_next
            .checked_sub(1i64.days())
            .map(|d| d.day())
            .unwrap_or(28),
        Err(_) => 28,
    }
}
