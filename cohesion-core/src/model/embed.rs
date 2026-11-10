//! An item a `cohesion://` link names, resolved for showing in place: one
//! shape for every kind, so a client can show a link to any item as a card.

use cohesion_macros::{TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::{
    ActorSession, Article, ArticleSummary, Contact, Database, DatabaseProperty, DatabaseRowEntry,
    Document, DocumentType, IntelligenceAgent, ItemLink, Resource, ResourceContentKind, Task,
};

/// How many of a database's rows its embed holds, the first in the order
/// they were added: enough for a card to preview it as a table, never the
/// whole of a large one.
pub const DATABASE_PREVIEW_ROWS: u64 = 5;

/// What kind of item an embed shows; `vocabulary.embed_kinds` words each.
///
/// - `"task"`, `"document"`, `"journalEntry"` (a document of type
///   `journal`), `"resource"`, `"database"`, `"databaseRow"`, `"contact"`,
///   `"agent"`, `"actorSession"`, `"article"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum EmbedKind {
    Task,
    Document,
    JournalEntry,
    Resource,
    Database,
    DatabaseRow,
    Contact,
    Agent,
    ActorSession,
    Article,
}

/// The item a `cohesion://` link names, as `resolveLink` returns it: what
/// every kind has (its title, the start of its text), which a client
/// shows for a kind it doesn't know, then the item itself in `item`.
#[derive(Clone, Serialize, Deserialize, TsWire)]
pub struct Embed {
    /// The item's link written in full, with its workspace:
    /// `cohesion://task/3/42` even when asked for as `cohesion://task/42`.
    pub link: String,
    pub kind: EmbedKind,
    /// The item's workspace; `null` for an agent, an actor session or an
    /// article, which belong to none.
    pub workspace_id: Option<i64>,
    /// The item's title; an agent's name; a contact's full name
    /// (`Contact::full_name`); a database row's label
    /// (`DatabaseRowEntry::label`).
    pub title: String,
    /// The start of the item's text as plain text, at most about 200
    /// characters: a task's description, a document's or text resource's
    /// contents, a database row's document's contents, a contact's notes,
    /// an agent's role, an
    /// actor session's last message from its actor, an article's text. `""`
    /// when it has none, as for a database.
    pub excerpt: String,
    pub item: EmbedItem,
}

/// The item an embed shows, tagged by `kind`: the model a client reads
/// it as elsewhere, so it can show more than the title and excerpt.
#[derive(Clone, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EmbedItem {
    Task {
        task: Task,
    },
    /// A document or a journal entry.
    Document {
        document: Document,
    },
    Resource {
        resource: Resource,
    },
    /// A database, with enough to preview it as a table.
    Database {
        database: Database,
        /// Its properties in their order.
        properties: Vec<DatabaseProperty>,
        /// Its first rows in the order they were added, at most
        /// `DATABASE_PREVIEW_ROWS` (5).
        rows: Vec<DatabaseRowEntry>,
        /// How many rows it has in all.
        row_count: i64,
    },
    /// A row of a database, with its database's properties, so its cells
    /// read as name and value; `row.document_id` is its document's.
    DatabaseRow {
        row: DatabaseRowEntry,
        database_title: String,
        /// Its database's properties in their order.
        properties: Vec<DatabaseProperty>,
    },
    Contact {
        contact: Contact,
    },
    Agent {
        agent: IntelligenceAgent,
    },
    ActorSession {
        session: ActorSession,
        /// Its actor's title, `null` when no client offers it any longer.
        actor_title: Option<String>,
    },
    Article {
        article: ArticleSummary,
        /// The title of the feed it came from.
        feed_title: String,
    },
}

impl Embed {
    pub fn task(task: Task) -> Self {
        Self {
            link: ItemLink::Task {
                workspace_id: Some(task.workspace_id),
                id: task.id,
            }
            .to_string(),
            kind: EmbedKind::Task,
            workspace_id: Some(task.workspace_id),
            title: task.title.clone(),
            excerpt: excerpt_after_title(&task.description, &task.title),
            item: EmbedItem::Task { task },
        }
    }

    pub fn document(document: Document) -> Self {
        Self {
            link: ItemLink::Document {
                workspace_id: Some(document.workspace_id),
                id: document.id,
            }
            .to_string(),
            kind: match document.document_type {
                DocumentType::Normal | DocumentType::DatabaseRow => EmbedKind::Document,
                DocumentType::Journal => EmbedKind::JournalEntry,
            },
            workspace_id: Some(document.workspace_id),
            title: document.title.clone(),
            excerpt: excerpt_after_title(&document.contents, &document.title),
            item: EmbedItem::Document { document },
        }
    }

    pub fn resource(resource: Resource) -> Self {
        Self {
            link: ItemLink::Resource {
                workspace_id: resource.workspace_id,
                id: resource.id,
            }
            .to_string(),
            kind: EmbedKind::Resource,
            workspace_id: Some(resource.workspace_id),
            title: resource.title.clone(),
            excerpt: match resource.content_kind {
                ResourceContentKind::Json => String::new(),
                ResourceContentKind::Markdown | ResourceContentKind::Text => {
                    excerpt_after_title(&resource.contents, &resource.title)
                }
            },
            item: EmbedItem::Resource { resource },
        }
    }

    /// Database `database` with its `properties`, its first `rows` and how
    /// many it has, `row_count`.
    pub fn database(
        database: Database,
        properties: Vec<DatabaseProperty>,
        rows: Vec<DatabaseRowEntry>,
        row_count: i64,
    ) -> Self {
        Self {
            link: ItemLink::Database {
                workspace_id: database.workspace_id,
                id: database.id,
            }
            .to_string(),
            kind: EmbedKind::Database,
            workspace_id: Some(database.workspace_id),
            title: database.title.clone(),
            excerpt: String::new(),
            item: EmbedItem::Database {
                database,
                properties,
                rows,
                row_count,
            },
        }
    }

    /// Row `row` of `database`, whose properties are `properties`, and the
    /// row's `document` if it has one.
    pub fn database_row(
        database: Database,
        properties: Vec<DatabaseProperty>,
        row: DatabaseRowEntry,
        document: Option<&Document>,
    ) -> Self {
        Self {
            link: ItemLink::DatabaseRow {
                workspace_id: database.workspace_id,
                database_id: database.id,
                id: row.id,
            }
            .to_string(),
            kind: EmbedKind::DatabaseRow,
            workspace_id: Some(database.workspace_id),
            title: row.label(&properties),
            excerpt: document
                .map(|document| excerpt(&document.contents))
                .unwrap_or_default(),
            item: EmbedItem::DatabaseRow {
                row,
                database_title: database.title,
                properties,
            },
        }
    }

    pub fn contact(contact: Contact) -> Self {
        Self {
            link: ItemLink::Contact {
                workspace_id: contact.workspace_id,
                id: contact.id,
            }
            .to_string(),
            kind: EmbedKind::Contact,
            workspace_id: Some(contact.workspace_id),
            title: contact.full_name(),
            excerpt: excerpt(&contact.notes),
            item: EmbedItem::Contact { contact },
        }
    }

    pub fn agent(agent: IntelligenceAgent) -> Self {
        Self {
            link: ItemLink::Agent { id: agent.id }.to_string(),
            kind: EmbedKind::Agent,
            workspace_id: None,
            title: agent.agent_name.clone(),
            excerpt: excerpt(&agent.agent_role),
            item: EmbedItem::Agent { agent },
        }
    }

    /// Actor session `session`, run by the actor titled `actor_title`, whose
    /// actor last said `latest`.
    pub fn actor_session(
        session: ActorSession,
        actor_title: Option<String>,
        latest: Option<&str>,
    ) -> Self {
        Self {
            link: ItemLink::ActorSession { id: session.id }.to_string(),
            kind: EmbedKind::ActorSession,
            workspace_id: None,
            title: session.title.clone(),
            excerpt: latest.map(excerpt).unwrap_or_default(),
            item: EmbedItem::ActorSession {
                session,
                actor_title,
            },
        }
    }

    /// Article `article`, from the feed titled `feed_title`.
    pub fn article(article: Article, feed_title: String) -> Self {
        Self {
            link: ItemLink::Article { id: article.id }.to_string(),
            kind: EmbedKind::Article,
            workspace_id: None,
            title: article.title.clone(),
            excerpt: excerpt_after_title(&article.contents, &article.title),
            item: EmbedItem::Article {
                article: article.into(),
                feed_title,
            },
        }
    }
}

/// About how many characters an excerpt holds.
const EXCERPT_CHARS: usize = 200;

/// The first paragraph of Markdown `text` that isn't a heading, a rule, a
/// table or code, as plain text: block and inline marks dropped, a link
/// as its text, cut at a word near `EXCERPT_CHARS` with an ellipsis.
pub fn excerpt(text: &str) -> String {
    excerpt_after_title(text, "")
}

/// As [`excerpt`], passing over a paragraph that only says `title` again,
/// as an article's text often starts.
fn excerpt_after_title(text: &str, title: &str) -> String {
    let title = word_key(title);
    let repeats = |words: &str| !title.is_empty() && word_key(words) == title;
    let mut words = String::new();
    let mut fenced = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if line.is_empty() {
            if words.is_empty() {
                continue;
            }
            if repeats(&words) {
                words.clear();
                continue;
            }
            break;
        }
        if line.starts_with('#') || line.starts_with('|') || is_rule(line) {
            continue;
        }
        let plain = plain(block_text(line));
        if plain.is_empty() {
            continue;
        }
        if !words.is_empty() {
            words.push(' ');
        }
        words.push_str(&plain);
        if words.chars().count() > EXCERPT_CHARS {
            break;
        }
    }
    if repeats(&words) {
        words.clear();
    }
    shortened(&words)
}

/// A thematic break: three or more of one of `-`, `*`, `_`, spaces aside.
fn is_rule(line: &str) -> bool {
    let mut marks = line.chars().filter(|c| !c.is_whitespace());
    let Some(mark @ ('-' | '*' | '_')) = marks.next() else {
        return false;
    };
    let rest: Vec<char> = marks.collect();
    rest.len() >= 2 && rest.iter().all(|&c| c == mark)
}

/// `text`'s words, lowercased, without the punctuation and symbols between
/// them: what a title and a paragraph repeating it share.
fn word_key(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// `line` without its block marks: quotes, list marks and task boxes.
fn block_text(mut line: &str) -> &str {
    loop {
        let before = line;
        line = line.trim_start();
        if let Some(rest) = line.strip_prefix('>') {
            line = rest;
        }
        for mark in ["- ", "* ", "+ ", "[ ] ", "[x] ", "[X] "] {
            if let Some(rest) = line.strip_prefix(mark) {
                line = rest;
            }
        }
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            let rest = &line[digits..];
            if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
                line = rest;
            }
        }
        if line == before {
            return line;
        }
    }
}

/// Inline Markdown as plain text: a link or image as its text, emphasis,
/// strikethrough and code marks dropped, runs of space as one.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if c == '!' && rest[1..].starts_with('[') {
            rest = &rest[1..];
            continue;
        }
        if c == '['
            && let Some((text, after)) = link_text(rest)
        {
            out.push_str(&plain(text));
            rest = after;
            continue;
        }
        if let Some(after) = ["**", "__", "~~", "*", "`"]
            .iter()
            .find_map(|mark| rest.strip_prefix(mark))
        {
            rest = after;
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[text](target)` at the start of `rest`: its text, and what follows it.
fn link_text(rest: &str) -> Option<(&str, &str)> {
    let close = rest.find("](")?;
    let end = rest[close + 2..].find(')')? + close + 2;
    Some((&rest[1..close], &rest[end + 1..]))
}

/// `words` cut at the last space within `EXCERPT_CHARS`, with an ellipsis.
fn shortened(words: &str) -> String {
    if words.chars().count() <= EXCERPT_CHARS {
        return words.to_string();
    }
    let cut: String = words.chars().take(EXCERPT_CHARS).collect();
    let cut = match cut.rfind(' ') {
        Some(space) if space > EXCERPT_CHARS / 2 => &cut[..space],
        _ => &cut,
    };
    format!(
        "{}…",
        cut.trim_end_matches(|c: char| c.is_ascii_punctuation())
    )
}

#[cfg(test)]
mod tests;
