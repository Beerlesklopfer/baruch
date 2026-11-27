// SPDX-License-Identifier: AGPL-3.0-or-later
//! Points in time for display in the **local time** of the machine (user decision 2026-11-07, UTC before).
//!
//! UTC keeps being stored (`strftime('%Y-%m-%dT%H:%M:%fZ','now')` in SQLite, `models::iso_now`,
//! Unix seconds); RPC fields stay UTC. Conversion happens only here, when the page is built. The time zone
//! comes from `chrono::Local` (under Unix `TZ`, otherwise `/etc/localtime`, with daylight saving time per point in
//! time; under Windows the time zone of the system through the Windows interface – built there, but not tried).
//!
//! Every function exists as `…_in(…, &zone)` for an arbitrary time zone (tests with `FixedOffset`, no
//! changing of `TZ` in the shared test process) and without the suffix for local time. Unreadable stamps
//! stay as they are (nothing invented).

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};

/// Stored UTC point in time (`2026-11-07T17:53:42.451Z`, without fractions, without `Z`, with a space
/// instead of `T` or with an offset) → point in time; otherwise `None`.
pub fn parse_utc(stamp: &str) -> Option<DateTime<Utc>> {
    let s = stamp.trim();
    if let Ok(at) = DateTime::parse_from_rfc3339(s) {
        return Some(at.with_timezone(&Utc));
    }
    let bare = s.trim_end_matches('Z').replacen(' ', "T", 1);
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(&bare, f).ok())
        .map(|naive| naive.and_utc())
}

fn format_in<Tz: TimeZone>(stamp: &str, zone: &Tz, pattern: &str) -> Option<String>
where
    Tz::Offset: std::fmt::Display,
{
    parse_utc(stamp).map(|at| at.with_timezone(zone).format(pattern).to_string())
}

/// `2026-11-05T10:11:12.123Z` → `2026-11-05 11:11` in `zone`; anything unreadable shortened as before.
pub fn minutes_in<Tz: TimeZone>(stamp: &str, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format_in(stamp, zone, "%Y-%m-%d %H:%M").unwrap_or_else(|| stamp.replacen('T', " ", 1).chars().take(16).collect())
}

/// `2026-11-05T10:11:12.123Z` → `2026-11-05 11:11` (local time).
pub fn minutes(stamp: &str) -> String {
    minutes_in(stamp, &Local)
}

/// `2026-11-06T12:05:06.123Z` → `2026-11-06 13:05:06` in `zone`; anything unreadable stays.
pub fn seconds_in<Tz: TimeZone>(stamp: &str, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format_in(stamp, zone, "%Y-%m-%d %H:%M:%S").unwrap_or_else(|| stamp.to_owned())
}

/// `2026-11-06T12:05:06.123Z` → `2026-11-06 13:05:06` (local time).
pub fn seconds(stamp: &str) -> String {
    seconds_in(stamp, &Local)
}

/// `2026-11-07T17:53:42.451Z` → `07.11.2026 18:53` in `zone`; anything unreadable stays.
pub fn german_minutes_in<Tz: TimeZone>(stamp: &str, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format_in(stamp, zone, "%d.%m.%Y %H:%M").unwrap_or_else(|| stamp.to_owned())
}

/// `2026-11-07T17:53:42.451Z` → `07.11.2026 18:53` (local time).
pub fn german_minutes(stamp: &str) -> String {
    german_minutes_in(stamp, &Local)
}

/// `2026-11-07T23:30:00Z` → `08.11.2026` in `zone` (the day can be a different one in local time); anything
/// unreadable stays.
pub fn german_day_in<Tz: TimeZone>(stamp: &str, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format_in(stamp, zone, "%d.%m.%Y").unwrap_or_else(|| stamp.to_owned())
}

/// `2026-11-07T23:30:00Z` → day `TT.MM.JJJJ` in local time.
pub fn german_day(stamp: &str) -> String {
    german_day_in(stamp, &Local)
}

/// Unix seconds → `JJJJ-MM-TT HH:MM:SS` in `zone`.
pub fn unix_seconds_in<Tz: TimeZone>(secs: u64, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    i64::try_from(secs)
        .ok()
        .and_then(|s| Utc.timestamp_opt(s, 0).single())
        .map_or_else(|| secs.to_string(), |at| at.with_timezone(zone).format("%Y-%m-%d %H:%M:%S").to_string())
}

/// Unix seconds → `JJJJ-MM-TT HH:MM:SS` (local time).
pub fn unix_seconds(secs: u64) -> String {
    unix_seconds_in(secs, &Local)
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn berlin_winter() -> FixedOffset {
        FixedOffset::east_opt(3600).expect("eine Stunde Versatz ist gültig")
    }

    fn berlin_summer() -> FixedOffset {
        FixedOffset::east_opt(7200).expect("zwei Stunden Versatz sind gültig")
    }

    #[test]
    fn stored_stamps_are_read_in_every_form_baruch_writes() {
        let want = Utc.with_ymd_and_hms(2026, 11, 7, 17, 53, 42).single();
        assert_eq!(parse_utc("2026-11-07T17:53:42Z"), want, "Websuche/Modelle schreiben ohne Bruchteile");
        assert_eq!(parse_utc("2026-11-07 17:53:42"), want, "SQLite datetime() mit Leerzeichen");
        assert_eq!(parse_utc("2026-11-07T18:53:42+01:00"), want, "Stempel mit Versatz werden nach UTC zurückgerechnet");
        assert!(
            parse_utc("2026-11-07T17:53:42.451Z").is_some_and(|at| at.timestamp() == want.unwrap().timestamp()),
            "strftime('%Y-%m-%dT%H:%M:%fZ') mit Millisekunden"
        );
        assert_eq!(parse_utc("gestern"), None, "Unlesbares ist kein Zeitpunkt");
    }

    #[test]
    fn display_times_follow_the_zone_including_summer_time() {
        assert_eq!(minutes_in("2026-11-05T10:11:12.123Z", &berlin_winter()), "2026-11-05 11:11", "Winterzeit +1");
        assert_eq!(minutes_in("2026-07-05T10:11:12.123Z", &berlin_summer()), "2026-07-05 12:11", "Sommerzeit +2");
        assert_eq!(seconds_in("2026-11-06T12:05:06.123Z", &berlin_winter()), "2026-11-06 13:05:06", "Sekunden");
        assert_eq!(german_minutes_in("2026-11-07T17:53:42.451Z", &berlin_winter()), "07.11.2026 18:53", "deutsch");
        assert_eq!(unix_seconds_in(1_793_966_706, &berlin_winter()), "2026-11-06 13:05:06", "Unix-Sekunden");
        assert_eq!(unix_seconds_in(0, &Utc), "1970-01-01 00:00:00", "UTC selbst bleibt UTC");
    }

    #[test]
    fn the_day_of_a_late_utc_stamp_is_the_next_local_day() {
        assert_eq!(
            german_day_in("2026-11-07T23:30:00Z", &berlin_winter()),
            "08.11.2026",
            "23:30 UTC ist in Berlin schon der nächste Tag – das Abrufdatum muss der Tag des Nutzers sein"
        );
        assert_eq!(german_day_in("2026-11-07T23:30:00Z", &Utc), "07.11.2026", "in UTC derselbe Tag");
    }

    #[test]
    fn unreadable_stamps_stay_as_they_are() {
        assert_eq!(minutes_in("kurz", &Utc), "kurz", "unbekannte Form bleibt (gekürzt wie bisher)");
        assert_eq!(seconds_in("kurz", &Utc), "kurz", "unbekannte Form bleibt");
        assert_eq!(german_minutes_in("gestern", &Utc), "gestern", "nichts erfunden");
        assert_eq!(german_day_in("", &Utc), "", "leer bleibt leer");
    }

    /// `chrono::Local` reads `TZ`. No environment variable is changed in the shared test process:
    /// the test starts itself as a child process with `TZ=Europe/Berlin` and checks the local time there.
    #[test]
    fn local_time_uses_the_zone_of_the_machine() {
        const CHILD: &str = "BARUCH_LOCALTIME_CHILD";
        if std::env::var_os(CHILD).is_some() {
            assert_eq!(minutes("2026-11-05T10:11:12.123Z"), "2026-11-05 11:11", "Berlin im Winter: +1 Stunde");
            assert_eq!(minutes("2026-07-05T10:11:12.123Z"), "2026-07-05 12:11", "Berlin im Sommer: +2 Stunden");
            assert_eq!(german_day("2026-11-07T23:30:00Z"), "08.11.2026", "Tag des Nutzers");
            return;
        }
        if !std::path::Path::new("/usr/share/zoneinfo/Europe/Berlin").exists() {
            eprintln!("ÜBERSPRUNGEN: keine Zeitzonen-Datenbank unter /usr/share/zoneinfo");
            return;
        }
        let exe = std::env::current_exe().expect("eigene Testdatei");
        let out = std::process::Command::new(exe)
            .args(["--exact", "localtime::tests::local_time_uses_the_zone_of_the_machine", "--test-threads=1"])
            .env(CHILD, "1")
            .env("TZ", "Europe/Berlin")
            .output()
            .expect("Kindprozess startet");
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "Ortszeit mit TZ=Europe/Berlin falsch: {text}");
        assert!(text.contains("1 passed"), "der Kindprozess muss genau diesen Test ausführen: {text}");
    }
}
