//! Natural-language time & date parsing for the leading tokens of a command.
//!
//! Input like `4pm tomorrow london toronto` is split into a `When` (the time
//! and/or date) and the remaining city tokens. Times/dates always come first;
//! the first token that isn't a recognizable time/date begins the city list.

use jiff::civil::{Date, Time, Weekday};

#[derive(Debug, Default, Clone)]
pub struct When {
    pub time: Option<Time>,
    pub date: Option<DateSpec>,
}

impl When {
    pub fn is_empty(&self) -> bool {
        self.time.is_none() && self.date.is_none()
    }
}

#[derive(Debug, Clone)]
pub enum DateSpec {
    Absolute(Date),
    RelativeDays(i64),
    Weekday { wd: Weekday, next_week: bool },
    MonthDay { month: i8, day: i8 },
}

impl DateSpec {
    /// Echo the spec back the way it was written, for error messages.
    pub fn describe(&self) -> String {
        match self {
            DateSpec::Absolute(d) => d.to_string(),
            DateSpec::RelativeDays(n) => format!("{n:+} days"),
            DateSpec::Weekday { wd, .. } => format!("{wd:?}").to_lowercase(),
            DateSpec::MonthDay { month, day } => {
                const M: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun",
                                       "jul", "aug", "sep", "oct", "nov", "dec"];
                let name = M.get((*month as usize).wrapping_sub(1)).copied().unwrap_or("?");
                format!("{name} {day}")
            }
        }
    }
}

/// Resolve a `DateSpec` against the anchor's current date.
/// `None` means the spec names no real date, e.g. `feb 30`. The caller must
/// reject it rather than substitute something plausible.
pub fn resolve_date(spec: &DateSpec, today: Date) -> Option<Date> {
    match spec {
        DateSpec::Absolute(d) => Some(*d),
        DateSpec::RelativeDays(n) => today.checked_add(jiff::Span::new().days(*n)).ok(),
        DateSpec::Weekday { wd, next_week } => {
            // Days until the target weekday (0 = today).
            let cur = today.weekday().to_monday_zero_offset() as i64;
            let tgt = wd.to_monday_zero_offset() as i64;
            let mut delta = (tgt - cur).rem_euclid(7);
            // "next monday" means the coming week: never today, roll 0 -> 7.
            if *next_week && delta == 0 {
                delta = 7;
            }
            today.checked_add(jiff::Span::new().days(delta)).ok()
        }
        DateSpec::MonthDay { month, day } => {
            // Search forward for the next occurrence, and never clamp. The
            // previous version fell back to the 28th of the month, so "feb 30"
            // silently answered with Feb 28 of the current year, a date in the
            // past, and "feb 29" gave the 28th rather than the next leap year.
            // An impossible day must stay unresolved so the caller can reject
            // it; a rare but real one should find the year where it exists.
            let start = today.year();
            (start..=start + 8)
                .filter_map(|y| Date::new(y, *month, *day).ok())
                .find(|d| *d >= today)
        }
    }
}

/// Pull leading time/date tokens off the front. Returns (when, remaining cities).
pub fn extract_when(tokens: &[String]) -> (When, Vec<String>) {
    let mut when = When::default();
    let lower: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
    let mut i = 0;
    while i < tokens.len() {
        let t = &lower[i];

        // "next <weekday>"
        if when.date.is_none() && t == "next" && i + 1 < tokens.len() {
            if let Some(wd) = parse_weekday(&lower[i + 1]) {
                when.date = Some(DateSpec::Weekday { wd, next_week: true });
                i += 2;
                continue;
            }
        }

        // month + day  ("july 1", "jul 1")  OR  day + month ("1 july")
        if when.date.is_none() && i + 1 < tokens.len() {
            if let Some(m) = parse_month(t) {
                if let Some(d) = parse_day_num(&lower[i + 1]) {
                    when.date = Some(DateSpec::MonthDay { month: m, day: d });
                    i += 2;
                    continue;
                }
            }
            if let Some(d) = parse_day_num(t) {
                if let Some(m) = parse_month(&lower[i + 1]) {
                    when.date = Some(DateSpec::MonthDay { month: m, day: d });
                    i += 2;
                    continue;
                }
            }
        }

        // single date token
        if when.date.is_none() {
            if let Some(ds) = parse_date_single(t) {
                when.date = Some(ds);
                i += 1;
                continue;
            }
        }

        // time token
        if when.time.is_none() {
            if let Some(tm) = parse_time(t) {
                when.time = Some(tm);
                i += 1;
                continue;
            }
        }

        break;
    }
    let cities = tokens[i..].to_vec();
    (when, cities)
}

fn parse_date_single(t: &str) -> Option<DateSpec> {
    match t {
        "today" | "tod" | "now" => return Some(DateSpec::RelativeDays(0)),
        "tomorrow" | "tom" | "tmrw" | "tmr" => return Some(DateSpec::RelativeDays(1)),
        "yesterday" | "yest" | "yd" => return Some(DateSpec::RelativeDays(-1)),
        _ => {}
    }
    if let Some(wd) = parse_weekday(t) {
        return Some(DateSpec::Weekday { wd, next_week: false });
    }
    // ISO 8601
    if let Ok(d) = t.parse::<Date>() {
        return Some(DateSpec::Absolute(d));
    }
    // M/D or M/D/Y  (and D.M.Y with dots is uncommon; handle slashes)
    if t.contains('/') {
        let parts: Vec<&str> = t.split('/').collect();
        if parts.len() == 2 || parts.len() == 3 {
            let m: i8 = parts[0].parse().ok()?;
            let d: i8 = parts[1].parse().ok()?;
            if parts.len() == 3 {
                let mut y: i16 = parts[2].parse().ok()?;
                if y < 100 {
                    y += 2000;
                }
                // Terminal on purpose. If the user named a year, that year is
                // the answer or there is no answer; falling through to the
                // month/day search would silently pick a different one.
                return Date::new(y, m, d).ok().map(DateSpec::Absolute);
            }
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return Some(DateSpec::MonthDay { month: m, day: d });
            }
        }
    }
    None
}

fn parse_weekday(t: &str) -> Option<Weekday> {
    let w = match t {
        "mon" | "monday" => Weekday::Monday,
        "tue" | "tues" | "tuesday" => Weekday::Tuesday,
        "wed" | "weds" | "wednesday" => Weekday::Wednesday,
        "thu" | "thur" | "thurs" | "thursday" => Weekday::Thursday,
        "fri" | "friday" => Weekday::Friday,
        "sat" | "saturday" => Weekday::Saturday,
        "sun" | "sunday" => Weekday::Sunday,
        _ => return None,
    };
    Some(w)
}

fn parse_month(t: &str) -> Option<i8> {
    let m = match t {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    };
    Some(m)
}

fn parse_day_num(t: &str) -> Option<i8> {
    // Accept "1", "01", "1st", "2nd", "3rd", "21st".
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let suffix = &t[digits.len()..];
    if !suffix.is_empty() && !matches!(suffix, "st" | "nd" | "rd" | "th") {
        return None;
    }
    let n: i8 = digits.parse().ok()?;
    if (1..=31).contains(&n) {
        Some(n)
    } else {
        None
    }
}

/// Parse a time token: 4pm, 4:30pm, 16:00, 16, 9am, noon, midnight, 1600.
pub fn parse_time(t: &str) -> Option<Time> {
    match t {
        "noon" | "midday" => return Time::new(12, 0, 0, 0).ok(),
        "midnight" => return Time::new(0, 0, 0, 0).ok(),
        _ => {}
    }

    let (body, ampm) = if let Some(b) = t.strip_suffix("am") {
        (b, Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b, Some(true))
    } else if let Some(b) = t.strip_suffix('a') {
        (b, Some(false))
    } else if let Some(b) = t.strip_suffix('p') {
        (b, Some(true))
    } else {
        (t, None)
    };
    let body = body.trim();

    // Accept both ':' and '.' as the hour/minute separator (4:30pm / 4.30pm).
    let (h_str, m_str) = if let Some((h, m)) = body.split_once(|c| c == ':' || c == '.') {
        (h, m)
    } else {
        (body, "")
    };

    if h_str.is_empty() || !h_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !m_str.is_empty() && !m_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    // Military "1600" / "0930" form (4 digits, no colon, no am/pm).
    if ampm.is_none() && m_str.is_empty() && h_str.len() == 4 {
        let h: i8 = h_str[..2].parse().ok()?;
        let m: i8 = h_str[2..].parse().ok()?;
        if (0..=23).contains(&h) && (0..=59).contains(&m) {
            return Time::new(h, m, 0, 0).ok();
        }
        return None;
    }

    let mut hour: i8 = h_str.parse().ok()?;
    let minute: i8 = if m_str.is_empty() { 0 } else { m_str.parse().ok()? };
    if !(0..=59).contains(&minute) {
        return None;
    }

    match ampm {
        Some(is_pm) => {
            if !(1..=12).contains(&hour) {
                return None;
            }
            if is_pm && hour != 12 {
                hour += 12;
            }
            if !is_pm && hour == 12 {
                hour = 0;
            }
        }
        None => {
            if !(0..=23).contains(&hour) {
                return None;
            }
        }
    }

    Time::new(hour, minute, 0, 0).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_dates_stay_unresolved() {
        // The failure worth guarding is not a crash, it is a confident answer
        // about a date the user never asked about.
        let today = Date::new(2026, 8, 13).unwrap();
        for (m, d) in [(2, 30), (11, 31), (4, 31)] {
            let spec = DateSpec::MonthDay { month: m, day: d };
            assert_eq!(resolve_date(&spec, today), None, "{m}/{d} resolved");
        }
        // Rare but real must still find the year in which it exists.
        assert_eq!(resolve_date(&DateSpec::MonthDay { month: 2, day: 29 }, today),
                   Some(Date::new(2028, 2, 29).unwrap()));
        // An explicitly typed year is never traded for a different one.
        assert!(parse_date_single("2/29/2027").is_none(), "bad year fell through");
        assert!(matches!(parse_date_single("2/28/2027"),
                         Some(DateSpec::Absolute(d)) if d == Date::new(2027, 2, 28).unwrap()));
    }

    fn hm(s: &str) -> Option<(i8, i8)> {
        parse_time(s).map(|t| (t.hour(), t.minute()))
    }

    #[test]
    fn parses_times() {
        // colon and dot separators are equivalent
        assert_eq!(hm("4:30pm"), Some((16, 30)));
        assert_eq!(hm("4.30pm"), Some((16, 30)));
        assert_eq!(hm("9.15am"), Some((9, 15)));
        assert_eq!(hm("16.00"), Some((16, 0)));
        assert_eq!(hm("16:00"), Some((16, 0)));
        // bare / am-pm / words / military
        assert_eq!(hm("4pm"), Some((16, 0)));
        assert_eq!(hm("12am"), Some((0, 0)));
        assert_eq!(hm("12pm"), Some((12, 0)));
        assert_eq!(hm("1600"), Some((16, 0)));
        assert_eq!(hm("noon"), Some((12, 0)));
        assert_eq!(hm("midnight"), Some((0, 0)));
        // not times
        assert_eq!(hm("london"), None);
        assert_eq!(hm("25:00"), None);
        assert_eq!(hm("4:99"), None);
    }
}
