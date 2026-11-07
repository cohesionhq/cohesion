//! Reading the date a page or feed says it was published, in whichever of
//! the web's formats it is written.

use cohesion_core::model::{Published, PublishedPrecision};
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::fmt::rfc2822;
use jiff::tz::TimeZone;

const DAY: i64 = 24 * 60 * 60;

/// `text` as a time of publication: an RFC 3339 / ISO 8601 timestamp
/// (pages' meta tags and Atom), an RFC 2822 date (RSS), or a bare
/// `YYYY-MM-DD` date. A bare date, or a timestamp at exactly midnight UTC
/// (how feeds without times write one), is only that day. `None` when it is
/// none of these.
pub fn published_time(text: &str) -> Option<Published> {
    let text = text.trim();
    let timestamp = text
        .parse::<Timestamp>()
        .ok()
        .or_else(|| rfc2822::parse(text).ok().map(|zoned| zoned.timestamp()))
        .or_else(|| {
            let date = text.parse::<Date>().ok()?;
            date.to_zoned(TimeZone::UTC)
                .ok()
                .map(|zoned| zoned.timestamp())
        })?;
    let at = timestamp.as_second();
    let precision = if at.rem_euclid(DAY) == 0 {
        PublishedPrecision::Day
    } else {
        PublishedPrecision::Time
    };
    Some(Published { at, precision })
}

#[cfg(test)]
mod tests;
