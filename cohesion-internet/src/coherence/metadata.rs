//! What a page says of itself: its title, author and date of publication,
//! from its meta tags, its JSON-LD and, failing those, its markup.

use cohesion_core::model::Published;
use dom_query::Document;
use serde_json::Value;

use crate::published_time;

/// What a page says of its article.
#[derive(Debug, Default)]
pub(crate) struct Metadata {
    /// Its title, whitespace collapsed; `None` when it has none or it is
    /// blank.
    pub title: Option<String>,
    /// Who wrote it, from its byline, meta tags or JSON-LD.
    pub author: Option<String>,
    /// When it was published, from its meta tags or JSON-LD.
    pub published: Option<Published>,
}

/// `page`'s metadata, read before anything of it is removed.
pub(super) fn metadata(page: &Document) -> Metadata {
    let site = meta(page, &["og:site_name", "application-name"]);
    let title = meta(page, &["og:title", "twitter:title"])
        .or_else(|| json_ld(page, &|object| text(object.get("headline")?)))
        .or_else(|| {
            let title = collapsed(&page.select("head title").text());
            // A heading the title opens with is the title without the
            // site's name after it.
            let heading = page
                .select("h1")
                .nodes()
                .iter()
                .map(|heading| collapsed(&heading.text()))
                .find(|heading| !heading.is_empty() && title.starts_with(heading.as_str()));
            heading.or_else(|| (!title.is_empty()).then_some(title))
        })
        .map(|title| without_site(&collapsed(&title), site.as_deref()))
        .filter(|title| !title.is_empty());
    let author = json_ld(page, &|object| names(object.get("author")?))
        .or_else(|| {
            meta(
                page,
                &["author", "article:author", "parsely-author", "dc.creator"],
            )
            .filter(|author| !author.contains("://"))
        })
        .or_else(|| {
            page.select("[rel~=author], [itemprop~=author]")
                .nodes()
                .iter()
                .map(|node| collapsed(&node.text()))
                .find(|name| !name.is_empty())
        })
        .and_then(|author| byline_names(&author));
    let published = meta(
        page,
        &[
            "article:published_time",
            "datepublished",
            "date",
            "pubdate",
            "publishdate",
            "dc.date",
            "dc.date.issued",
            "parsely-pub-date",
        ],
    )
    .and_then(|date| published_time(&date))
    .or_else(|| json_ld_published(page))
    .or_else(|| {
        page.select("time[datetime]")
            .nodes()
            .iter()
            .find_map(|time| published_time(&time.attr_or("datetime", "")))
    });
    Metadata {
        title,
        author,
        published,
    }
}

/// The image the page says stands for it when shared.
pub(super) fn image(page: &Document) -> Option<String> {
    meta(page, &["og:image", "twitter:image"])
}

/// What the page says it is about, each way it says it (for sharing, and
/// for search), its paragraphs kept; the first is the one to show.
pub(super) fn descriptions(page: &Document) -> Vec<String> {
    let nodes = page.select("meta[content]").nodes().to_vec();
    let mut descriptions: Vec<String> = Vec::new();
    for key in ["og:description", "twitter:description", "description"] {
        for node in &nodes {
            let named = ["property", "name"]
                .iter()
                .any(|attribute| node.attr_or(attribute, "").trim().eq_ignore_ascii_case(key));
            let content = node.attr_or("content", "").trim().to_string();
            if named && !content.is_empty() && !descriptions.contains(&content) {
                descriptions.push(content);
            }
        }
    }
    descriptions
}

/// The first non-blank `content` of a `<meta>` whose `property`, `name` or
/// `itemprop` is one of `keys`, whatever its case.
fn meta(page: &Document, keys: &[&str]) -> Option<String> {
    let nodes = page.select("meta[content]").nodes().to_vec();
    keys.iter().find_map(|key| {
        nodes.iter().find_map(|node| {
            let named = ["property", "name", "itemprop"]
                .iter()
                .any(|attribute| node.attr_or(attribute, "").trim().eq_ignore_ascii_case(key));
            let content = collapsed(&node.attr_or("content", ""));
            (named && !content.is_empty()).then_some(content)
        })
    })
}

/// `title` without the site's name a page appends to it, as in
/// `Sourdough | The Gazette`.
fn without_site(title: &str, site: Option<&str>) -> String {
    let Some(site) = site else {
        return title.to_string();
    };
    for separator in [" | ", " - ", " – ", " — ", " · ", " :: "] {
        if let Some(rest) = title.strip_suffix(&format!("{separator}{site}")) {
            return rest.trim().to_string();
        }
    }
    title.to_string()
}

/// The first of the page's JSON-LD objects, those in an `@graph` included,
/// that `read` reads something from.
fn json_ld<T>(
    page: &Document,
    read: &dyn Fn(&serde_json::Map<String, Value>) -> Option<T>,
) -> Option<T> {
    fn find<T>(
        value: &Value,
        read: &dyn Fn(&serde_json::Map<String, Value>) -> Option<T>,
    ) -> Option<T> {
        match value {
            Value::Array(values) => values.iter().find_map(|value| find(value, read)),
            Value::Object(object) => read(object).or_else(|| find(object.get("@graph")?, read)),
            _ => None,
        }
    }
    page.select(r#"script[type="application/ld+json"]"#)
        .nodes()
        .iter()
        .filter_map(|script| serde_json::from_str(&script.text()).ok())
        .find_map(|value: Value| find(&value, read))
}

/// When the page's JSON-LD says it was published: the first
/// `datePublished` among its objects, those in an `@graph` included, which
/// is where WordPress's SEO plugins put the article.
fn json_ld_published(page: &Document) -> Option<Published> {
    json_ld(page, &|object| {
        published_time(object.get("datePublished")?.as_str()?)
    })
}

/// A JSON-LD text, collapsed; `None` when blank.
fn text(value: &Value) -> Option<String> {
    Some(collapsed(value.as_str()?)).filter(|text| !text.is_empty())
}

/// The names a JSON-LD `author` gives: a name, a person, or a list of
/// either, joined.
fn names(value: &Value) -> Option<String> {
    let name = |value: &Value| match value {
        Value::String(_) => text(value),
        Value::Object(person) => text(person.get("name")?),
        _ => None,
    };
    let names: Vec<String> = match value {
        Value::Array(values) => values.iter().filter_map(name).collect(),
        value => name(value).into_iter().collect(),
    };
    (!names.is_empty()).then(|| names.join(", "))
}

/// The names a byline gives: a leading "By" or "Written by" dropped, and
/// `None` when what is left reads as more than names, such as a dateline
/// with the time and a count of comments.
fn byline_names(byline: &str) -> Option<String> {
    /// The most words a list of names runs to.
    const NAMES_WORDS: usize = 10;
    let words: Vec<&str> = byline.split_whitespace().collect();
    let is = |word: &str, expected: &str| word.trim_end_matches(':').eq_ignore_ascii_case(expected);
    let names = match words.as_slice() {
        [by, rest @ ..] if is(by, "by") => rest,
        [verb, by, rest @ ..]
            if is(by, "by")
                && ["written", "posted", "words", "story"]
                    .iter()
                    .any(|v| is(verb, v)) =>
        {
            rest
        }
        all => all,
    };
    let dated = names
        .iter()
        .any(|word| word.contains(|c: char| c.is_ascii_digit()));
    (!names.is_empty() && names.len() <= NAMES_WORDS && !dated).then(|| names.join(" "))
}

/// `text` with its whitespace collapsed to single spaces and trimmed.
pub(super) fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
