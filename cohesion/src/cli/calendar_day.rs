//! Days as the daemon stores task dates: `YYYY-MM-DD`, with no time zone.

use jiff::{Zoned, civil::Date};

/// A day given on the command line, or `none` to clear one.
#[derive(Clone)]
pub struct DayArg(pub Option<String>);

/// Today in the local time zone, as `YYYY-MM-DD`.
pub fn today() -> String {
    Zoned::now().date().to_string()
}

/// Parses `YYYY-MM-DD`, `today` or `tomorrow` (in the local time zone) into
/// a `YYYY-MM-DD` day.
pub fn parse_day(text: &str) -> Result<String, String> {
    let today = Zoned::now().date();
    let date = match text {
        "today" => today,
        "tomorrow" => today.tomorrow().map_err(|error| error.to_string())?,
        _ => text
            .parse::<Date>()
            .map_err(|_| "expected YYYY-MM-DD, today or tomorrow".to_string())?,
    };
    Ok(date.to_string())
}

/// Like `parse_day`, but `none` clears the day.
pub fn parse_day_arg(text: &str) -> Result<DayArg, String> {
    match text {
        "none" => Ok(DayArg(None)),
        _ => parse_day(text).map(|day| DayArg(Some(day))),
    }
}
