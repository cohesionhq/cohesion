use cohesion_core::model::{Published, PublishedPrecision};

use super::published_time;

fn at(at: i64) -> Option<Published> {
    Some(Published {
        at,
        precision: PublishedPrecision::Time,
    })
}

fn on(at: i64) -> Option<Published> {
    Some(Published {
        at,
        precision: PublishedPrecision::Day,
    })
}

#[test]
fn reads_the_formats_pages_and_feeds_use() {
    assert_eq!(published_time("2024-03-01T12:00:00Z"), at(1_709_294_400));
    assert_eq!(
        published_time("2024-03-01T13:00:00+01:00"),
        at(1_709_294_400)
    );
    assert_eq!(
        published_time("Fri, 01 Mar 2024 12:00:00 GMT"),
        at(1_709_294_400)
    );
    assert_eq!(published_time("last Tuesday"), None);
}

#[test]
fn a_date_without_a_time_is_a_day() {
    assert_eq!(published_time(" 2024-03-01 "), on(1_709_251_200));
    assert_eq!(
        published_time("Fri, 01 Mar 2024 00:00:00 GMT"),
        on(1_709_251_200)
    );
    // Midnight somewhere else is a moment.
    assert_eq!(
        published_time("2024-03-01T00:00:00-06:00"),
        at(1_709_272_800)
    );
}
