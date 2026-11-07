//! What Coherence reads off a document once, before deciding any rule: how
//! much text each node holds and how much of it is links, which elements
//! hold an image or a block, and each element's names. Rules never change
//! the document, so none of it goes stale while the page is read.

use std::collections::{HashMap, HashSet};

use dom_query::{Document, NodeId, NodeRef};

use super::rules::STATE_WORDS;

/// A document and what [`Page::new`] read off it.
pub(crate) struct Page<'a> {
    pub document: &'a Document,
    /// Every node, the document first, each before what it holds.
    nodes: Vec<NodeRef<'a>>,
    /// The text each node holds, in non-whitespace bytes.
    text: HashMap<NodeId, usize>,
    /// How much of that text sits in links, a link's own included.
    linked: HashMap<NodeId, usize>,
    /// Images, and what holds one.
    images: HashSet<NodeId>,
    /// Elements that hold a block (see [`BLOCKS`]) below them.
    blocks: HashSet<NodeId>,
    /// The elements rules going by names read, in document order, each
    /// with its names as lowercased parts; the page and its article, and
    /// names that state a page's state or only style an element, left out.
    named: Vec<(NodeRef<'a>, Vec<Vec<Box<str>>>)>,
}

/// Elements that hold a block of their own, so a `div` holding one isn't
/// itself a paragraph.
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "ul",
    "ol",
    "dl",
    "table",
    "pre",
    "blockquote",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "footer",
    "aside",
    "main",
    "nav",
];

impl<'a> Page<'a> {
    pub fn new(document: &'a Document) -> Self {
        let root = document.root();
        let nodes: Vec<NodeRef<'a>> = std::iter::once(root).chain(root.descendants_it()).collect();
        let mut page = Self {
            document,
            nodes,
            text: HashMap::new(),
            linked: HashMap::new(),
            images: HashSet::new(),
            blocks: HashSet::new(),
            named: Vec::new(),
        };
        // Last to first, so what a node holds is read before it.
        for node in page.nodes.iter().rev() {
            if node.is_text() {
                let size = node
                    .text()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .map(char::len_utf8)
                    .sum();
                page.text.insert(node.id, size);
                continue;
            }
            let (mut text, mut linked) = (0, 0);
            let (mut image, mut block) = (node.has_name("img"), false);
            for child in node.children_it(false) {
                text += page.text.get(&child.id).copied().unwrap_or_default();
                linked += page.linked.get(&child.id).copied().unwrap_or_default();
                image |= page.images.contains(&child.id);
                block |= page.blocks.contains(&child.id) || is_block(&child);
            }
            if node.has_name("a") {
                linked = text;
            }
            page.text.insert(node.id, text);
            page.linked.insert(node.id, linked);
            if image {
                page.images.insert(node.id);
            }
            if block {
                page.blocks.insert(node.id);
            }
        }
        page.named = page
            .nodes
            .iter()
            .filter(|node| node.is_element() && !is_page_or_article(node))
            .filter_map(|node| {
                let names = names(node);
                (!names.is_empty()).then_some((*node, names))
            })
            .collect();
        page
    }

    /// Every node, each before what it holds.
    pub fn nodes(&self) -> &[NodeRef<'a>] {
        &self.nodes
    }

    /// The elements with names rules read, and those names.
    pub fn named(&self) -> &[(NodeRef<'a>, Vec<Vec<Box<str>>>)] {
        &self.named
    }

    /// How much text `node` holds: its bytes, whitespace aside.
    pub fn text(&self, node: &NodeRef) -> usize {
        self.text.get(&node.id).copied().unwrap_or_default()
    }

    /// How much of `element`'s text is in the links it holds, from 0 to 1.
    pub fn link_density(&self, element: &NodeRef) -> f64 {
        let size = self.text(element);
        if size == 0 {
            return 0.0;
        }
        let linked: usize = element
            .children_it(false)
            .map(|child| self.linked.get(&child.id).copied().unwrap_or_default())
            .sum();
        linked as f64 / size as f64
    }

    /// Whether `element` holds an image.
    pub fn holds_image(&self, element: &NodeRef) -> bool {
        element
            .children_it(false)
            .any(|child| self.images.contains(&child.id))
    }

    /// Whether `element` holds a block element.
    pub fn holds_block(&self, element: &NodeRef) -> bool {
        self.blocks.contains(&element.id)
    }
}

fn is_block(node: &NodeRef) -> bool {
    BLOCKS.iter().any(|name| node.has_name(name))
}

/// Whether `node` is the page or its article, which a rule going by names
/// never picks.
pub(crate) fn is_page_or_article(node: &NodeRef) -> bool {
    ["html", "body", "article", "main"]
        .iter()
        .any(|name| node.has_name(name))
}

/// Each of `node`'s `class` and `id` names, as its lowercased parts, but
/// those that open or close with one of [`STATE_WORDS`], and utility
/// classes such as `xl:[--sidebar-wide:1]`, which style an element rather
/// than name it.
fn names(node: &NodeRef) -> Vec<Vec<Box<str>>> {
    let class = node.attr_or("class", "").to_ascii_lowercase();
    let id = node.attr_or("id", "").to_ascii_lowercase();
    class
        .split_whitespace()
        .chain(id.split_whitespace())
        .filter(|name| !name.contains([':', '[']))
        .map(|name| {
            name.split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|part| !part.is_empty())
                .map(Box::from)
                .collect::<Vec<_>>()
        })
        .filter(|parts| {
            let state =
                |part: Option<&Box<str>>| part.is_some_and(|part| STATE_WORDS.contains(&&**part));
            !parts.is_empty() && !(state(parts.first()) || state(parts.last()))
        })
        .collect()
}
