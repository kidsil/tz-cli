//! City / timezone name resolution.
//!
//! Turns human input ("london", "nyc", "pst", "Europe/Madrid") into an IANA
//! `TimeZone`. Strategy, in order:
//!   1. Curated alias table (handles the common cities, abbreviations and the
//!      ambiguous-name disambiguation that fuzzy matching would get wrong).
//!   2. Direct IANA lookup if the input already looks like "Area/Location".
//!   3. Fuzzy match against the last path component of every IANA zone name
//!      (so "toronto" -> "America/Toronto", "kolkata" -> "Asia/Kolkata", and
//!      typos like "torrento" still land).

use jiff::tz::TimeZone;

/// A resolved zone, carrying a friendly display label.
pub struct Resolved {
    pub iana: String,
    pub display: String,
    pub tz: TimeZone,
}

/// Curated aliases. Keys are normalized (lowercase, no spaces/underscores).
/// This is where we encode "london means the UK one" and common shorthand.
const ALIASES: &[(&str, &str)] = &[
    // --- abbreviations & UTC ---
    ("utc", "UTC"),
    ("gmt", "Etc/GMT"),
    ("z", "UTC"),
    ("zulu", "UTC"),
    ("est", "America/New_York"),
    ("edt", "America/New_York"),
    ("et", "America/New_York"),
    ("eastern", "America/New_York"),
    ("cst", "America/Chicago"),
    ("cdt", "America/Chicago"),
    ("ct", "America/Chicago"),
    ("central", "America/Chicago"),
    ("mst", "America/Denver"),
    ("mdt", "America/Denver"),
    ("mt", "America/Denver"),
    ("mountain", "America/Denver"),
    ("pst", "America/Los_Angeles"),
    ("pdt", "America/Los_Angeles"),
    ("pt", "America/Los_Angeles"),
    ("pacific", "America/Los_Angeles"),
    ("cet", "Europe/Paris"),
    ("cest", "Europe/Paris"),
    ("eet", "Europe/Athens"),
    ("bst", "Europe/London"),
    ("ist", "Asia/Kolkata"), // India Standard Time (most common use)
    ("jst", "Asia/Tokyo"),
    ("kst", "Asia/Seoul"),
    ("aest", "Australia/Sydney"),
    ("aedt", "Australia/Sydney"),
    ("awst", "Australia/Perth"),
    ("nzst", "Pacific/Auckland"),
    ("nzdt", "Pacific/Auckland"),
    // --- cities that fuzzy would get wrong or that are very common ---
    ("london", "Europe/London"),
    ("nyc", "America/New_York"),
    ("newyork", "America/New_York"),
    ("ny", "America/New_York"),
    ("sf", "America/Los_Angeles"),
    ("sanfrancisco", "America/Los_Angeles"),
    ("la", "America/Los_Angeles"),
    ("losangeles", "America/Los_Angeles"),
    ("seattle", "America/Los_Angeles"),
    ("vegas", "America/Los_Angeles"),
    ("dc", "America/New_York"),
    ("washington", "America/New_York"),
    ("boston", "America/New_York"),
    ("miami", "America/New_York"),
    ("atlanta", "America/New_York"),
    ("dallas", "America/Chicago"),
    ("houston", "America/Chicago"),
    ("austin", "America/Chicago"),
    ("denver", "America/Denver"),
    ("phoenix", "America/Phoenix"),
    ("chi", "America/Chicago"),
    ("philly", "America/New_York"),
    ("toronto", "America/Toronto"),
    ("montreal", "America/Toronto"),
    ("ottawa", "America/Toronto"),
    ("vancouver", "America/Vancouver"),
    ("mexicocity", "America/Mexico_City"),
    ("cdmx", "America/Mexico_City"),
    ("saopaulo", "America/Sao_Paulo"),
    ("rio", "America/Sao_Paulo"),
    ("buenosaires", "America/Argentina/Buenos_Aires"),
    ("ba", "America/Argentina/Buenos_Aires"),
    ("santiago", "America/Santiago"),
    ("bogota", "America/Bogota"),
    ("lima", "America/Lima"),
    // Europe
    ("paris", "Europe/Paris"),
    ("berlin", "Europe/Berlin"),
    ("madrid", "Europe/Madrid"),
    ("barcelona", "Europe/Madrid"),
    ("rome", "Europe/Rome"),
    ("milan", "Europe/Rome"),
    ("amsterdam", "Europe/Amsterdam"),
    ("brussels", "Europe/Brussels"),
    ("zurich", "Europe/Zurich"),
    ("geneva", "Europe/Zurich"),
    ("vienna", "Europe/Vienna"),
    ("munich", "Europe/Berlin"),
    ("frankfurt", "Europe/Berlin"),
    ("lisbon", "Europe/Lisbon"),
    ("dublin", "Europe/Dublin"),
    ("copenhagen", "Europe/Copenhagen"),
    ("oslo", "Europe/Oslo"),
    ("stockholm", "Europe/Stockholm"),
    ("helsinki", "Europe/Helsinki"),
    ("warsaw", "Europe/Warsaw"),
    ("prague", "Europe/Prague"),
    ("budapest", "Europe/Budapest"),
    ("athens", "Europe/Athens"),
    ("istanbul", "Europe/Istanbul"),
    ("moscow", "Europe/Moscow"),
    ("kyiv", "Europe/Kyiv"),
    ("kiev", "Europe/Kyiv"),
    // Middle East / Africa
    ("dubai", "Asia/Dubai"),
    ("abudhabi", "Asia/Dubai"),
    ("telaviv", "Asia/Jerusalem"),
    ("jerusalem", "Asia/Jerusalem"),
    ("israel", "Asia/Jerusalem"),
    ("riyadh", "Asia/Riyadh"),
    ("doha", "Asia/Qatar"),
    ("cairo", "Africa/Cairo"),
    ("lagos", "Africa/Lagos"),
    ("nairobi", "Africa/Nairobi"),
    ("johannesburg", "Africa/Johannesburg"),
    ("joburg", "Africa/Johannesburg"),
    ("capetown", "Africa/Johannesburg"),
    ("casablanca", "Africa/Casablanca"),
    // Asia
    ("delhi", "Asia/Kolkata"),
    ("newdelhi", "Asia/Kolkata"),
    ("mumbai", "Asia/Kolkata"),
    ("bangalore", "Asia/Kolkata"),
    ("bengaluru", "Asia/Kolkata"),
    ("kolkata", "Asia/Kolkata"),
    ("india", "Asia/Kolkata"),
    ("karachi", "Asia/Karachi"),
    ("dhaka", "Asia/Dhaka"),
    ("kathmandu", "Asia/Kathmandu"),
    ("colombo", "Asia/Colombo"),
    ("bangkok", "Asia/Bangkok"),
    ("jakarta", "Asia/Jakarta"),
    ("singapore", "Asia/Singapore"),
    ("kl", "Asia/Kuala_Lumpur"),
    ("kualalumpur", "Asia/Kuala_Lumpur"),
    ("manila", "Asia/Manila"),
    ("hongkong", "Asia/Hong_Kong"),
    ("hk", "Asia/Hong_Kong"),
    ("shanghai", "Asia/Shanghai"),
    ("beijing", "Asia/Shanghai"),
    ("china", "Asia/Shanghai"),
    ("shenzhen", "Asia/Shanghai"),
    ("taipei", "Asia/Taipei"),
    ("seoul", "Asia/Seoul"),
    ("tokyo", "Asia/Tokyo"),
    ("osaka", "Asia/Tokyo"),
    ("japan", "Asia/Tokyo"),
    // Oceania
    ("sydney", "Australia/Sydney"),
    ("melbourne", "Australia/Melbourne"),
    ("brisbane", "Australia/Brisbane"),
    ("perth", "Australia/Perth"),
    ("adelaide", "Australia/Adelaide"),
    ("auckland", "Pacific/Auckland"),
    ("wellington", "Pacific/Auckland"),
    ("nz", "Pacific/Auckland"),
    ("honolulu", "Pacific/Honolulu"),
    ("hawaii", "Pacific/Honolulu"),
    // Countries -> a sensible default city (usually the capital / most populous).
    ("uk", "Europe/London"),
    ("england", "Europe/London"),
    ("britain", "Europe/London"),
    ("greatbritain", "Europe/London"),
    ("unitedkingdom", "Europe/London"),
    ("scotland", "Europe/London"),
    ("ireland", "Europe/Dublin"),
    ("france", "Europe/Paris"),
    ("germany", "Europe/Berlin"),
    ("spain", "Europe/Madrid"),
    ("italy", "Europe/Rome"),
    ("portugal", "Europe/Lisbon"),
    ("netherlands", "Europe/Amsterdam"),
    ("holland", "Europe/Amsterdam"),
    ("belgium", "Europe/Brussels"),
    ("switzerland", "Europe/Zurich"),
    ("austria", "Europe/Vienna"),
    ("poland", "Europe/Warsaw"),
    ("czechia", "Europe/Prague"),
    ("czechrepublic", "Europe/Prague"),
    ("hungary", "Europe/Budapest"),
    ("romania", "Europe/Bucharest"),
    ("greece", "Europe/Athens"),
    ("denmark", "Europe/Copenhagen"),
    ("norway", "Europe/Oslo"),
    ("sweden", "Europe/Stockholm"),
    ("finland", "Europe/Helsinki"),
    ("turkey", "Europe/Istanbul"),
    ("russia", "Europe/Moscow"),
    ("ukraine", "Europe/Kyiv"),
    ("usa", "America/New_York"),
    ("us", "America/New_York"),
    ("america", "America/New_York"),
    ("unitedstates", "America/New_York"),
    ("canada", "America/Toronto"),
    ("mexico", "America/Mexico_City"),
    ("brazil", "America/Sao_Paulo"),
    ("argentina", "America/Argentina/Buenos_Aires"),
    ("chile", "America/Santiago"),
    ("colombia", "America/Bogota"),
    ("peru", "America/Lima"),
    ("egypt", "Africa/Cairo"),
    ("nigeria", "Africa/Lagos"),
    ("kenya", "Africa/Nairobi"),
    ("southafrica", "Africa/Johannesburg"),
    ("morocco", "Africa/Casablanca"),
    ("uae", "Asia/Dubai"),
    ("emirates", "Asia/Dubai"),
    ("saudiarabia", "Asia/Riyadh"),
    ("saudi", "Asia/Riyadh"),
    ("qatar", "Asia/Qatar"),
    ("iran", "Asia/Tehran"),
    ("iraq", "Asia/Baghdad"),
    ("pakistan", "Asia/Karachi"),
    ("bangladesh", "Asia/Dhaka"),
    ("nepal", "Asia/Kathmandu"),
    ("srilanka", "Asia/Colombo"),
    ("thailand", "Asia/Bangkok"),
    ("vietnam", "Asia/Ho_Chi_Minh"),
    ("hanoi", "Asia/Ho_Chi_Minh"),
    ("saigon", "Asia/Ho_Chi_Minh"),
    ("hochiminh", "Asia/Ho_Chi_Minh"),
    ("indonesia", "Asia/Jakarta"),
    ("malaysia", "Asia/Kuala_Lumpur"),
    ("philippines", "Asia/Manila"),
    ("korea", "Asia/Seoul"),
    ("southkorea", "Asia/Seoul"),
    ("taiwan", "Asia/Taipei"),
    ("australia", "Australia/Sydney"),
    ("newzealand", "Pacific/Auckland"),
];

/// Normalize input for alias lookup: lowercase, drop spaces/underscores/dots.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '_' | '.' | '-' | '\''))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Pretty display name from an IANA id, e.g. "America/New_York" -> "New York".
fn display_from_iana(iana: &str) -> String {
    let last = iana.rsplit('/').next().unwrap_or(iana);
    last.replace('_', " ")
}

/// Build a `Resolved` directly from a known IANA id (used for stored history).
pub fn from_iana(iana: &str) -> Option<Resolved> {
    TimeZone::get(iana).ok().map(|tz| Resolved {
        iana: iana.to_string(),
        display: display_from_iana(iana),
        tz,
    })
}

/// Resolve user input to a zone: cheap exact match first, then fuzzy fallback.
/// Returns None if nothing plausible matches.
pub fn resolve(input: &str) -> Option<Resolved> {
    resolve_exact(input).or_else(|| resolve_fuzzy(input))
}

/// Cheap, scan-free resolution: the local zone, a curated alias, or a direct
/// IANA id. Used on its own for multi-word phrases, where running a full fuzzy
/// scan for every word-combination would be wasteful (and slow).
pub fn resolve_exact(input: &str) -> Option<Resolved> {
    let norm = normalize(input);
    if norm.is_empty() {
        return None;
    }

    // The local / system zone.
    if matches!(norm.as_str(), "local" | "here" | "me" | "mine" | "system") {
        let tz = TimeZone::system();
        let iana = tz.iana_name().unwrap_or("Local").to_string();
        let display = if iana == "Local" {
            "Local".to_string()
        } else {
            display_from_iana(&iana)
        };
        return Some(Resolved { iana, display, tz });
    }

    // Curated alias.
    if let Some((_, iana)) = ALIASES.iter().find(|(k, _)| *k == norm) {
        if let Ok(tz) = TimeZone::get(iana) {
            return Some(Resolved {
                iana: iana.to_string(),
                display: display_from_iana(iana),
                tz,
            });
        }
    }

    // Direct IANA id (e.g. "Europe/Berlin", "UTC").
    if let Ok(tz) = TimeZone::get(input) {
        return Some(Resolved {
            iana: input.to_string(),
            display: display_from_iana(input),
            tz,
        });
    }

    None
}

/// Fuzzy match against the last path component of every IANA zone. Only reached
/// for single tokens that didn't match exactly (typos / uncommon cities).
fn resolve_fuzzy(input: &str) -> Option<Resolved> {
    let norm = normalize(input);
    if norm.is_empty() {
        return None;
    }

    let mut best: Option<(i32, String)> = None;
    for name in jiff::tz::db().available() {
        let iana = name.as_str();
        let last = iana.rsplit('/').next().unwrap_or(iana);
        let last_norm = normalize(last);
        // Exact last-component match can't be beaten — stop scanning.
        if last_norm == norm {
            return from_iana(iana);
        }
        if let Some(score) = match_score(&norm, &last_norm) {
            // Tie-break: prefer higher score, then shorter IANA id (more canonical).
            let better = match &best {
                None => true,
                Some((bs, bi)) => score > *bs || (score == *bs && iana.len() < bi.len()),
            };
            if better {
                best = Some((score, iana.to_string()));
            }
        }
    }

    if let Some((score, iana)) = best {
        // Require a reasonable match to avoid nonsense resolving to something.
        if score >= 55 {
            if let Ok(tz) = TimeZone::get(&iana) {
                return Some(Resolved {
                    iana: iana.clone(),
                    display: display_from_iana(&iana),
                    tz,
                });
            }
        }
    }

    None
}

/// Resolve a sequence of word-tokens into cities, greedily combining adjacent
/// words so multi-word names group correctly: `hong kong`, `new york`,
/// `los angeles`, `hong kong`. Returns `(original_phrase, Resolved)` per city,
/// or `Err(unresolved_words)`.
pub fn resolve_sequence(tokens: &[String]) -> Result<Vec<(String, Resolved)>, Vec<String>> {
    const MAX_WORDS: usize = 3;
    let mut out = Vec::new();
    let mut bad = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let max_k = MAX_WORDS.min(tokens.len() - i);
        let mut matched = false;
        // Prefer the longest phrase that resolves, so "hong kong" beats "hong".
        // Multi-word phrases use cheap (alias/IANA) resolution only — fuzzy
        // scanning every combination is slow and rarely intended; a single
        // token still gets the full fuzzy fallback.
        for k in (1..=max_k).rev() {
            let phrase = tokens[i..i + k].join(" ");
            let r = if k == 1 {
                resolve(&phrase)
            } else {
                resolve_exact(&phrase)
            };
            if let Some(r) = r {
                out.push((phrase, r));
                i += k;
                matched = true;
                break;
            }
        }
        if !matched {
            bad.push(tokens[i].clone());
            i += 1;
        }
    }
    if bad.is_empty() {
        Ok(out)
    } else {
        Err(bad)
    }
}

/// Score how well `query` matches `candidate` (both normalized). Higher better.
fn match_score(query: &str, candidate: &str) -> Option<i32> {
    if query == candidate {
        return Some(100);
    }
    // Require three characters before treating input as a prefix. Below that
    // nearly everything matches something: "i" scored Inuvik, "a" Aden. Genuine
    // short aliases (la, ny, sf, hk, uk, pst) are resolved exactly, before here.
    if query.len() >= 3 && candidate.starts_with(query) {
        // Closer in length is a better prefix match.
        return Some(85 - (candidate.len() as i32 - query.len() as i32).min(20));
    }
    if candidate.contains(query) && query.len() >= 4 {
        return Some(65);
    }
    // Typo tolerance via edit distance, but require a shared prefix so short or
    // unrelated words can't match far-off zones: "adis" must not become "Davis"
    // (no shared prefix), while "warsha" still finds "Warsaw" (shares "wars").
    // Words under 5 chars rely on exact/prefix/substring matches only.
    if query.len() >= 5 {
        // Require a shared prefix BEFORE the (relatively expensive) edit-distance
        // computation — this both prunes the scan and stops short/unrelated words
        // from matching far-off zones.
        let prefix = shared_prefix_len(query, candidate);
        if prefix >= 2 {
            let allowed = if query.len() >= 6 { 2 } else { 1 };
            let dist = levenshtein(query, candidate);
            let max_len = query.len().max(candidate.len()) as i32;
            let sim = 100 - (dist * 100 / max_len.max(1));
            if dist <= allowed && sim >= 60 {
                return Some(50 + sim / 3);
            }
        }
    }
    None
}

/// Length of the shared leading prefix of two strings (in chars).
fn shared_prefix_len(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

/// Classic Levenshtein edit distance.
fn levenshtein(a: &str, b: &str) -> i32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<i32> = (0..=b.len() as i32).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i as i32;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
