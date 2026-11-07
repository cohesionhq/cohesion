//! What a cleanup rule is, and how a list of them is decided over a
//! document. A rule is data: a [`Trigger`] that picks elements and an
//! action for each, built with `const fn`s so a list of them is a `static`
//! slice (see [`super::rules`]). Rules don't change the document: they are
//! decided over it once, in order, into [`Decisions`] that Coherence's
//! finder and writer read as they go. A new kind of case is a new variant
//! here, its picking in [`pick`] and its deciding in [`decide`].

use std::collections::{HashMap, HashSet};

use dom_query::{Matcher, NodeId, NodeRef};

use super::page::{Page, is_page_or_article};
use crate::article_snapshot::Url;

/// Elements picked out of a document and what is done to each.
pub(crate) struct Rule {
    trigger: Trigger,
    action: Action,
}

impl Rule {
    /// Removes what `trigger` picks.
    pub const fn delete(trigger: Trigger) -> Self {
        Self::new(trigger, Action::Delete)
    }

    /// Replaces what `trigger` picks with what it holds.
    pub const fn unwrap(trigger: Trigger) -> Self {
        Self::new(trigger, Action::Unwrap)
    }

    /// Removes the section each heading `trigger` picks titles: the heading,
    /// or the wrapper holding only it, and what follows up to the next
    /// heading of its rank or higher.
    pub const fn delete_section(trigger: Trigger) -> Self {
        Self::new(trigger, Action::DeleteSection)
    }

    /// Has no later rule going by names read anything into the names of
    /// what `trigger` picks.
    pub const fn strip_names(trigger: Trigger) -> Self {
        Self::new(trigger, Action::StripNames)
    }

    /// Replaces an embed `trigger` picks with a paragraph linking what
    /// `embed` reads it as; one it reads no address from stays.
    pub const fn link_to(trigger: Trigger, embed: Embed) -> Self {
        Self::new(trigger, Action::LinkTo(embed))
    }

    /// Gives an image `trigger` picks the address in its `attribute` as its
    /// `src`: a lazily loaded image's real address.
    pub const fn source_from(trigger: Trigger, attribute: &'static str) -> Self {
        Self::new(trigger, Action::SourceFrom(attribute))
    }

    const fn new(trigger: Trigger, action: Action) -> Self {
        Self { trigger, action }
    }
}

/// How much an element's being picked by a trigger counts for or against
/// it holding the article.
pub(crate) struct Hint {
    trigger: Trigger,
    weight: f64,
}

impl Hint {
    pub const fn favor(trigger: Trigger, weight: u32) -> Self {
        Self {
            trigger,
            weight: weight as f64,
        }
    }

    pub const fn against(trigger: Trigger, weight: u32) -> Self {
        Self {
            trigger,
            weight: -(weight as f64),
        }
    }
}

/// What `hints` add up to for each element of `page` one picks.
pub(crate) fn weights(page: &Page, hints: &[Hint], decisions: &Decisions) -> HashMap<NodeId, f64> {
    let mut weights = HashMap::new();
    for hint in hints {
        for node in picked(page, &hint.trigger, decisions) {
            *weights.entry(node.id).or_default() += hint.weight;
        }
    }
    weights
}

/// Which elements a [`Rule`] acts on.
pub(crate) enum Trigger {
    /// Those matching a CSS selector.
    Selector(&'static str),
    /// The element with this `id`, whatever its case.
    ElementWithId(&'static str),
    /// Those with this class, whatever its case.
    ElementWithClass(&'static str),
    /// Those one of whose `class` or `id` names [`Name`] matches, but never
    /// the page or its article.
    Named(Name),
    /// For each element matching the first CSS selector, the closest
    /// element around it matching the second: the block a byline's author
    /// link sits in, not every block holding one.
    Around(&'static str, &'static str),
    /// Headings whose text the [`Phrase`] matches.
    Heading(Phrase),
    /// Links whose text the [`Phrase`] matches.
    Link(Phrase),
    /// A block of links whose only other text is a label the [`Phrase`]
    /// matches, such as "Tags:" before a post's tags.
    LinkList(Phrase),
    /// Images declared no larger than this many CSS pixels on each side:
    /// icons, badges and tracking pixels.
    SmallImage(u32),
    /// Images whose address the [`Address`] matches.
    ImageFrom(Address),
    /// Links holding an image and no text.
    ImageLink,
    /// Links that are a glyph rather than text, such as a closing ★ or a
    /// footnote's ↩: no letter, digit or image, and either leading within
    /// the page (`#…`) or all their block holds.
    GlyphLink,
    /// Elements, but the page and its article, that hold an image and no
    /// text: a picture's wrappers.
    PictureWrapper,
    /// A script's code left as bare text directly in the body or a `div`:
    /// it assigns (`=`) something braced (`{`) and ends as a statement.
    BareCode,
    /// What any of these picks.
    Any(&'static [Trigger]),
}

/// How a [`Trigger::Named`] reads a `class` or `id` name, split into parts
/// at anything that isn't a letter or digit and lowercased, so
/// `club-call-to-action` reads as `club`, `call`, `to`, `action`.
pub(crate) enum Name {
    /// One part is one of these.
    Part(&'static [&'static str]),
    /// One part starts with one of these.
    Prefix(&'static [&'static str]),
    /// These parts come one after another.
    Run(&'static [&'static str]),
    /// One part is in the first list and another in the second.
    Pair(&'static [&'static str], &'static [&'static str]),
}

/// How a heading's or label's text reads: lowercased, whitespace
/// collapsed, punctuation at its ends dropped.
pub(crate) enum Phrase {
    /// It is exactly one of these.
    Is(&'static [&'static str]),
    StartsWith(&'static str),
    EndsWith(&'static str),
}

/// How a [`Trigger::ImageFrom`] reads an image's `src`.
pub(crate) enum Address {
    /// It is inlined as a `data:` URL: a placeholder.
    Inline,
    /// Its host ends with this.
    HostEndsWith(&'static str),
    /// Its host starts with this.
    HostStartsWith(&'static str),
    /// Its path has a folder named one of these.
    Folder(&'static [&'static str]),
}

/// Where an embed a [`Rule::link_to`] replaces leads.
#[derive(Clone, Copy)]
pub(crate) enum Embed {
    /// To the address in this attribute, such as an Instagram post's
    /// permalink.
    Permalink(&'static str),
    /// To the YouTube video whose id is in this attribute, such as a
    /// player's facade standing in until it is clicked.
    YouTube(&'static str),
}

impl Embed {
    /// Where `embed` leads, and the address to show as the link's text.
    pub fn link(&self, embed: &NodeRef) -> Option<(Url, String)> {
        match self {
            Embed::Permalink(attribute) => {
                let address = Url::parse(embed.attr(attribute)?.trim()).ok()?;
                let mut shown = address.clone();
                shown.set_query(None);
                shown.set_fragment(None);
                Some((address, shown.to_string()))
            }
            Embed::YouTube(attribute) => {
                let id = embed.attr(attribute)?;
                let id = id.trim();
                if id.is_empty()
                    || !id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
                {
                    return None;
                }
                let address = Url::parse(&format!("https://www.youtube.com/watch?v={id}")).ok()?;
                Some((address.clone(), address.to_string()))
            }
        }
    }
}

/// What a [`Rule`] does to what it picked.
enum Action {
    Delete,
    Unwrap,
    DeleteSection,
    StripNames,
    LinkTo(Embed),
    SourceFrom(&'static str),
}

/// What rules decided of a document's nodes.
#[derive(Default)]
pub(crate) struct Decisions {
    /// What is left out or rewritten; the first rule to decide a node wins.
    fates: HashMap<NodeId, Fate>,
    /// The attribute holding an image's real address.
    sources: HashMap<NodeId, &'static str>,
    /// What no rule going by names reads anything into.
    unnamed: HashSet<NodeId>,
}

/// What becomes of a node a rule decided.
#[derive(Clone, Copy)]
pub(crate) enum Fate {
    /// Left out, with all it holds.
    Deleted,
    /// Replaced by what it holds.
    Unwrapped,
    /// Replaced by a paragraph linking where the embed leads.
    LinkedTo(Embed),
}

impl Decisions {
    /// What becomes of `node`, when a rule decided.
    pub fn fate(&self, node: &NodeRef) -> Option<Fate> {
        self.fates.get(&node.id).copied()
    }

    /// Every node of `page` left out, or held by one that is.
    pub fn removed(&self, page: &Page) -> HashSet<NodeId> {
        let mut removed = HashSet::new();
        for node in page.nodes() {
            let held = node
                .parent()
                .is_some_and(|parent| removed.contains(&parent.id));
            if held || matches!(self.fate(node), Some(Fate::Deleted)) {
                removed.insert(node.id);
            }
        }
        removed
    }

    /// Leaves `node` out.
    pub fn delete(&mut self, node: &NodeRef) {
        self.fates.entry(node.id).or_insert(Fate::Deleted);
    }

    /// The address of `image`: its `src`, or the attribute a rule found
    /// its real address in.
    pub fn source(&self, image: &NodeRef) -> String {
        self.sources
            .get(&image.id)
            .and_then(|attribute| image.attr(attribute))
            .filter(|source| !source.trim().is_empty())
            .or_else(|| image.attr("src"))
            .map(|source| source.trim().to_string())
            .unwrap_or_default()
    }
}

/// Decides `rules` over `page`, in order.
pub(crate) fn decide(page: &Page, rules: &[Rule]) -> Decisions {
    let mut decisions = Decisions::default();
    for rule in rules {
        for node in picked(page, &rule.trigger, &decisions) {
            match rule.action {
                Action::Delete => decisions.delete(&node),
                Action::Unwrap => {
                    decisions.fates.entry(node.id).or_insert(Fate::Unwrapped);
                }
                Action::LinkTo(embed) => {
                    if embed.link(&node).is_some() {
                        decisions
                            .fates
                            .entry(node.id)
                            .or_insert(Fate::LinkedTo(embed));
                    }
                }
                Action::DeleteSection => {
                    for node in section(page, &node) {
                        decisions.delete(&node);
                    }
                }
                Action::StripNames => {
                    decisions.unnamed.insert(node.id);
                }
                Action::SourceFrom(attribute) => {
                    if node
                        .attr(attribute)
                        .is_some_and(|value| !value.trim().is_empty())
                    {
                        decisions.sources.insert(node.id, attribute);
                    }
                }
            }
        }
    }
    decisions
}

/// What `trigger` picks in `page`, each once.
fn picked<'a>(page: &Page<'a>, trigger: &Trigger, decisions: &Decisions) -> Vec<NodeRef<'a>> {
    let mut nodes = Vec::new();
    pick(page, trigger, decisions, &mut nodes);
    let mut seen = HashSet::<NodeId>::new();
    nodes.retain(|node| seen.insert(node.id));
    nodes
}

fn pick<'a>(
    page: &Page<'a>,
    trigger: &Trigger,
    decisions: &Decisions,
    into: &mut Vec<NodeRef<'a>>,
) {
    let select = |selector: &str| page.document.select(selector).nodes().to_vec();
    let filtered = |selector: &str, accepts: &dyn Fn(&NodeRef) -> bool| {
        page.document
            .select(selector)
            .nodes()
            .iter()
            .filter(|node| accepts(node))
            .copied()
            .collect::<Vec<_>>()
    };
    let found = match trigger {
        Trigger::Selector(selector) => select(selector),
        Trigger::ElementWithId(id) => filtered("[id]", &|node| {
            node.attr_or("id", "").trim().eq_ignore_ascii_case(id)
        }),
        Trigger::ElementWithClass(class) => filtered("[class]", &|node| {
            node.attr_or("class", "")
                .split_whitespace()
                .any(|name| name.eq_ignore_ascii_case(class))
        }),
        Trigger::Named(name) => page
            .named()
            .iter()
            .filter(|(node, names)| {
                !decisions.unnamed.contains(&node.id)
                    && names.iter().any(|parts| name.matches(parts))
            })
            .map(|(node, _)| *node)
            .collect(),
        Trigger::Around(inner, outer) => {
            let outer = Matcher::new(outer).expect("a rule's selector parses");
            select(inner)
                .into_iter()
                .filter_map(|node| {
                    node.ancestors_it(None)
                        .find(|ancestor| ancestor.is_match(&outer))
                })
                .collect()
        }
        Trigger::Heading(phrase) => filtered("h1, h2, h3, h4, h5, h6", &|heading| {
            phrase.matches(&heading.text())
        }),
        Trigger::Link(phrase) => filtered("a", &|link| phrase.matches(&link.text())),
        Trigger::LinkList(phrase) => {
            filtered("p, div, ul, section", &|block| is_link_list(block, phrase))
        }
        Trigger::SmallImage(size) => filtered("img", &|image| {
            let declared: Vec<u32> = ["width", "height"]
                .iter()
                .filter_map(|side| {
                    let value = image.attr(side)?;
                    value.trim().trim_end_matches("px").parse().ok()
                })
                .collect();
            !declared.is_empty() && declared.iter().all(|side| side <= size)
        }),
        Trigger::ImageFrom(address) => {
            filtered("img", &|image| address.matches(&decisions.source(image)))
        }
        Trigger::ImageLink => filtered("a", &|link| page.text(link) == 0 && page.holds_image(link)),
        Trigger::GlyphLink => filtered("a[href]", &|link| is_glyph_link(page, link)),
        Trigger::PictureWrapper => page
            .nodes()
            .iter()
            .filter(|node| {
                node.is_element()
                    && !is_page_or_article(node)
                    && page.text(node) == 0
                    && page.holds_image(node)
            })
            .copied()
            .collect(),
        Trigger::BareCode => select("body, div")
            .into_iter()
            .flat_map(|parent| parent.children())
            .filter(|child| child.is_text() && is_code(&child.text()))
            .collect(),
        Trigger::Any(triggers) => {
            for trigger in *triggers {
                pick(page, trigger, decisions, into);
            }
            return;
        }
    };
    into.extend(found);
}

impl Name {
    fn matches(&self, parts: &[Box<str>]) -> bool {
        let any = |list: &[&str]| parts.iter().any(|part| list.contains(&&**part));
        match self {
            Name::Part(list) => any(list),
            Name::Prefix(prefixes) => parts
                .iter()
                .any(|part| prefixes.iter().any(|prefix| part.starts_with(prefix))),
            Name::Run(run) => parts
                .windows(run.len())
                .any(|window| window.iter().map(|part| &**part).eq(run.iter().copied())),
            Name::Pair(first, second) => any(first) && any(second),
        }
    }
}

impl Phrase {
    fn matches(&self, text: &str) -> bool {
        let text = phrase(text);
        match self {
            Phrase::Is(list) => list.contains(&text.as_str()),
            Phrase::StartsWith(start) => text.starts_with(start),
            Phrase::EndsWith(end) => text.ends_with(end),
        }
    }
}

/// `text` as a [`Phrase`] reads it.
fn phrase(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

impl Address {
    fn matches(&self, source: &str) -> bool {
        if let Address::Inline = self {
            return source.trim_start().starts_with("data:");
        }
        let Ok(url) = Url::parse(source) else {
            return false;
        };
        let host = url.host_str().unwrap_or_default();
        match self {
            Address::Inline => unreachable!("handled above"),
            Address::HostEndsWith(end) => host.ends_with(end),
            Address::HostStartsWith(start) => host.starts_with(start),
            Address::Folder(folders) => url
                .path_segments()
                .is_some_and(|mut segments| segments.any(|segment| folders.contains(&segment))),
        }
    }
}

/// Whether `block` holds links and, outside them, only a label `phrase`
/// matches and separators.
fn is_link_list(block: &NodeRef, phrase: &Phrase) -> bool {
    /// Gathers the text outside links below `node` into `label`.
    fn walk(node: &NodeRef, label: &mut String) {
        for child in node.children_it(false) {
            if child.is_text() {
                label.push_str(&child.text());
                label.push(' ');
            } else if !child.has_name("a") {
                walk(&child, label);
            }
        }
    }
    if !block.descendants_it().any(|node| node.has_name("a")) {
        return false;
    }
    let mut label = String::new();
    walk(block, &mut label);
    let label = label
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || ",|·•/:".contains(c))
        .filter(|word| !word.is_empty() && *word != "and")
        .collect::<Vec<_>>()
        .join(" ");
    phrase.matches(&label)
}

/// Whether `link` is a glyph standing for a link back, not text. An arrow
/// beside a heading's text, such as `### GTK [↗](…)`, stays.
fn is_glyph_link(page: &Page, link: &NodeRef) -> bool {
    let wordless = !link.text().chars().any(char::is_alphanumeric) && !page.holds_image(link);
    wordless
        && (link.attr_or("href", "").starts_with('#')
            || link
                .parent()
                .is_some_and(|block| page.text(&block) == page.text(link)))
}

/// Whether bare `text` reads as a script's code.
fn is_code(text: &str) -> bool {
    let text = text.trim();
    text.contains('=') && text.contains('{') && text.ends_with([';', '}'])
}

/// The section `heading` titles: the heading, or the wrapper holding only
/// it, and what follows up to the next heading of its rank or higher.
fn section<'a>(page: &Page, heading: &NodeRef<'a>) -> Vec<NodeRef<'a>> {
    let rank = heading.node_name().unwrap_or_default();
    let higher = (1..=rank[1..].parse::<u8>().unwrap_or(6))
        .map(|level| format!("h{level}"))
        .collect::<Vec<_>>();
    let is_higher = |node: &NodeRef| higher.iter().any(|name| node.has_name(name));
    let mut start = *heading;
    while let Some(parent) = start.parent()
        && !is_page_or_article(&parent)
        && page.text(&parent) == page.text(heading)
    {
        start = parent;
    }
    let mut section = vec![start];
    let mut next = start.next_sibling();
    while let Some(node) = next {
        if is_higher(&node) || node.descendants_it().any(|inner| is_higher(&inner)) {
            break;
        }
        next = node.next_sibling();
        section.push(node);
    }
    section
}
