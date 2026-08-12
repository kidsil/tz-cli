//! Usage history: which cities you've checked, how often, and how recently.
//!
//! Stored at $XDG_CONFIG_HOME/tz/history as `iana<TAB>count<TAB>last_unix` per
//! line. Ranking uses a "frecency" score (frequency weighted by recency), so
//! the dashboard surfaces both your recently- and frequently-checked cities.
//! All operations are best-effort: any I/O error degrades to "no history".

use std::collections::HashMap;
use std::path::PathBuf;

use crate::cities::{self, Resolved};

struct Rec {
    count: u32,
    last: i64,
}

fn history_file() -> Option<PathBuf> {
    Some(config_dir()?.join("history"))
}

fn config_dir() -> Option<PathBuf> {
    crate::config::config_dir()
}

fn now_unix() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// Frequency weighted by how recently the city was last checked.
fn frecency(r: &Rec, now: i64) -> i64 {
    let age = (now - r.last).max(0);
    let weight = if age < 3_600 {
        16 // last hour
    } else if age < 86_400 {
        8 // last day
    } else if age < 7 * 86_400 {
        4 // last week
    } else if age < 30 * 86_400 {
        2 // last month
    } else {
        1
    };
    r.count as i64 * weight
}

fn load() -> HashMap<String, Rec> {
    let mut map = HashMap::new();
    let Some(path) = history_file() else {
        return map;
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let (Some(iana), Some(c), Some(l)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(count), Ok(last)) = (c.parse::<u32>(), l.parse::<i64>()) else {
            continue;
        };
        map.insert(iana.to_string(), Rec { count, last });
    }
    map
}

fn save(map: &HashMap<String, Rec>, now: i64) {
    let Some(path) = history_file() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut entries: Vec<(&String, &Rec)> = map.iter().collect();
    entries.sort_by(|a, b| frecency(b.1, now).cmp(&frecency(a.1, now)));
    entries.truncate(100); // keep the file bounded
    let mut body = String::from("# tz usage history: iana<TAB>count<TAB>last_unix\n");
    for (iana, r) in entries {
        body.push_str(&format!("{}\t{}\t{}\n", iana, r.count, r.last));
    }
    let _ = std::fs::write(path, body);
}

/// Record that these zones were just checked (increments count, bumps recency).
pub fn record(zones: &[Resolved]) {
    if zones.is_empty() {
        return;
    }
    let mut map = load();
    let now = now_unix();
    for z in zones {
        let e = map.entry(z.iana.clone()).or_insert(Rec { count: 0, last: now });
        e.count = e.count.saturating_add(1);
        e.last = now;
    }
    save(&map, now);
}

/// The top recent/frequent cities, excluding any IANA ids in `exclude`,
/// ranked by frecency. Returns (zone, times_checked).
pub fn recent(exclude: &[String], limit: usize) -> Vec<(Resolved, u32)> {
    let map = load();
    let now = now_unix();
    let mut v: Vec<(&String, &Rec)> = map
        .iter()
        .filter(|(iana, _)| !exclude.iter().any(|e| e == *iana))
        .collect();
    v.sort_by(|a, b| {
        frecency(b.1, now)
            .cmp(&frecency(a.1, now))
            .then(b.1.last.cmp(&a.1.last))
    });
    v.into_iter()
        .filter_map(|(iana, r)| cities::from_iana(iana).map(|z| (z, r.count)))
        .take(limit)
        .collect()
}
