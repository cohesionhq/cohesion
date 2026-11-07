//! Reading the text a feed's elements hold, whichever format it is in.

use dom_query::Document;
use quick_xml::Reader;
use quick_xml::events::Event;

/// The local name of `xml`'s root element.
pub(super) fn root_element(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event().ok()? {
            Event::Start(start) | Event::Empty(start) => {
                return Some(start.local_name().into_inner().to_string());
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// `text` trimmed, unless that leaves nothing.
pub(super) fn present(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// A title or name as plain text, unless that leaves nothing. Feeds write
/// these as HTML whatever their format says, escaping entities twice
/// (`&amp;#8217;`) or putting markup in a title, so the text is read as
/// HTML: entities decoded, tags dropped, whitespace collapsed.
pub(super) fn plain_text(text: Option<String>) -> Option<String> {
    let text = text?;
    let text = if text.contains(['<', '&']) {
        Document::fragment(text).root().text().to_string()
    } else {
        text
    };
    present(Some(text.split_whitespace().collect::<Vec<_>>().join(" ")))
}

/// `names` joined, each read as [`plain_text`]; `None` when none has one.
pub(super) fn joined_names(names: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let names = names.into_iter().filter_map(plain_text).collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(", "))
}
