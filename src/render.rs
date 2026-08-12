//! One-shot (print-and-exit) rendering of a set of zones at a given instant.

use std::io::IsTerminal;

use jiff::tz::AmbiguousOffset;
use jiff::{Timestamp, Zoned};

use crate::cities::Resolved;
use crate::parse::{resolve_date, When};

pub struct Options {
    pub use_24h: bool,
}

/// Compute the instant to display and any DST note, given the anchor zone.
/// Returns (instant, optional_dst_note).
fn compute_instant(
    when: &When,
    anchor: &Resolved,
) -> Result<(Timestamp, Option<String>), Box<dyn std::error::Error>> {
    if when.is_empty() {
        return Ok((Timestamp::now(), None));
    }

    let anchor_now = Timestamp::now().to_zoned(anchor.tz.clone());
    let date = match &when.date {
        Some(spec) => resolve_date(spec, anchor_now.date())
            .ok_or_else(|| format!("no such date: {}", spec.describe()))?,
        None => anchor_now.date(),
    };
    let time = when.time.unwrap_or_else(|| anchor_now.time());
    let dt = date.to_datetime(time);

    let az = anchor.tz.to_ambiguous_zoned(dt);
    let note = match az.offset() {
        AmbiguousOffset::Unambiguous { .. } => None,
        AmbiguousOffset::Gap { .. } => Some(format!(
            "{} doesn't exist in {} (DST spring-forward) — adjusted forward",
            dt.strftime("%-I:%M %p"),
            anchor.display
        )),
        AmbiguousOffset::Fold { .. } => Some(format!(
            "{} happens twice in {} (DST fall-back) — showing the first",
            dt.strftime("%-I:%M %p"),
            anchor.display
        )),
    };
    let zoned = az.compatible()?;
    Ok((zoned.timestamp(), note))
}

struct Row {
    is_anchor: bool,
    time: String,
    date: String,
    delta: String,
    city: String,
    abbr: String,
    offset: String,
}

pub fn render(
    zones: &[Resolved],
    when: &When,
    opts: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let anchor = &zones[0];
    let (instant, note) = compute_instant(when, anchor)?;

    // Reference date = anchor's local date at the instant, for day-delta markers.
    let ref_date = instant.to_zoned(anchor.tz.clone()).date();

    let time_fmt = if opts.use_24h { "%H:%M" } else { "%-I:%M %p" };

    // One labelling convention per table, decided once. Deciding per row put
    // an unlabelled "Thu Dec 31" (which is the previous year) directly beneath
    // two rows labelled 2027 - the exact ambiguity the year label exists to
    // remove.
    let now_year = jiff::Zoned::now().year();
    let show_year = zones
        .iter()
        .any(|z| instant.to_zoned(z.tz.clone()).year() != now_year);

    let mut rows = Vec::new();
    for z in zones {
        let zoned: Zoned = instant.to_zoned(z.tz.clone());
        let day_diff = days_between(ref_date, zoned.date());
        let delta = match day_diff {
            0 => String::new(),
            1 => "next day".into(),
            -1 => "prev day".into(),
            n if n > 0 => format!("+{n}d"),
            n => format!("{n}d"),
        };
        rows.push(Row {
            is_anchor: std::ptr::eq(z, anchor),
            time: zoned.strftime(time_fmt).to_string(),
            // A bare month-day resolves forward to the next year it exists
            // in, so "jul 1" in August can mean next July and "feb 29" the next
            // leap year. Without the year the only clue is the weekday.
            date: if show_year {
                zoned.strftime("%a %b %-d %Y").to_string()
            } else {
                zoned.strftime("%a %b %-d").to_string()
            },
            delta,
            city: z.display.clone(),
            abbr: zoned.strftime("%Z").to_string(),
            offset: format_offset(&zoned),
        });
    }

    // Match fx: honour the NO_COLOR convention and TERM=dumb, not just the tty.
    let color = std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map_or(true, |t| t != "dumb");
    print_rows(&rows, color);

    if let Some(note) = note {
        let warn = paint(&format!("  ⚠  {note}"), "33", color);
        println!("{warn}");
    }
    Ok(())
}

fn print_rows(rows: &[Row], color: bool) {
    let w_time = rows.iter().map(|r| r.time.len()).max().unwrap_or(0);
    let w_city = rows.iter().map(|r| r.city.len()).max().unwrap_or(0);
    let w_date = rows.iter().map(|r| r.date.len()).max().unwrap_or(0);
    let w_abbr = rows.iter().map(|r| r.abbr.len()).max().unwrap_or(0);

    println!();
    for r in rows {
        let marker = if r.is_anchor { "▶" } else { " " };
        let marker = paint(marker, "36", color);
        let time = pad(&r.time, w_time);
        let time = if r.is_anchor {
            paint_bold(&time, "1;36", color)
        } else {
            paint_bold(&time, "1;37", color)
        };
        let city = pad(&r.city, w_city);
        let city = paint(&city, "32", color);
        let date = pad(&r.date, w_date);
        let abbr = pad(&r.abbr, w_abbr);
        let meta = paint(&format!("{date}  {abbr}  {}", r.offset), "90", color);
        let delta = if r.delta.is_empty() {
            String::new()
        } else {
            format!("  {}", paint(&format!("({})", r.delta), "33", color))
        };
        println!("  {marker} {time}   {city}   {meta}{delta}");
    }
    println!();
}

/// Format the UTC offset like "UTC+1" or "UTC+5:30".
fn format_offset(z: &Zoned) -> String {
    let secs = z.offset().seconds();
    let sign = if secs < 0 { '-' } else { '+' };
    let secs = secs.abs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

fn days_between(a: jiff::civil::Date, b: jiff::civil::Date) -> i64 {
    // b - a in whole days.
    b.since(a).map(|s| s.get_days() as i64).unwrap_or(0)
}

fn pad(s: &str, w: usize) -> String {
    let mut out = s.to_string();
    while display_width(&out) < w {
        out.push(' ');
    }
    out
}

// Treat each char as width 1 for our ASCII-ish strings (good enough here).
fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn paint(s: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn paint_bold(s: &str, code: &str, color: bool) -> String {
    paint(s, code, color)
}
