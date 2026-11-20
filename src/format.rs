//! Numbers and times for stored data: always English and without thousands separators,
//! independent of the interface language – what a run writes into the history does not change
//! with it. Display formats via `i18n` (`count`, `size`, `date_time` …).

/// A point in time in local time, `format` as in strftime ("%d.%m.%Y %H:%M"). The one place for
/// date and time in local time; the patterns per language are in `i18n::Texts`.
pub(crate) fn local_time(time: jiff::Timestamp, format: &str) -> String {
    time.to_zoned_datetime(jiff::tz::TimeZone::system())
        .strftime(format)
        .to_string()
}

/// "1 file", "1234 files" – for the history (`Msg` with `{p}`).
pub(crate) fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}
