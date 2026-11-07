//! Finding the elements of a page that hold its article. Each paragraph no
//! rule left out counts toward the elements holding it, most for its own
//! block and less the further out; what [`super::rules::HINTS`] says of an
//! element's names, and how much of its text is links, weighs in; the best
//! scored element, with the siblings that read as more of it, is the
//! article.

use std::collections::{HashMap, HashSet};

use dom_query::{NodeId, NodeRef};

use super::metadata::collapsed;
use super::page::Page;
use super::rule::{Decisions, weights};
use super::rules::HINTS;

/// The least text, in non-whitespace bytes, a paragraph holds to count.
const PARAGRAPH_MIN: usize = 25;

/// The hint that doubles an element's score; its opposite halves it.
const HINT_SCALE: f64 = 50.0;

/// The least text a `div` holding blocks holds directly, outside its child
/// elements, to count as a paragraph itself.
const BARE_TEXT_MIN: usize = 100;

/// How many of the best scored elements are weighed for an ancestor they
/// share.
const RIVALS: usize = 5;

/// The least a sibling of the article's element scores to join it.
const SIBLING_MIN: f64 = 10.0;

/// How many levels out a paragraph counts toward.
const LEVELS: usize = 5;

/// What a page says of itself that its article may open with.
pub(super) struct Lead<'s> {
    pub title: Option<&'s str>,
    /// Each way the page describes itself.
    pub descriptions: &'s [String],
    pub image: Option<&'s str>,
}

/// The elements of `page` that hold its article, in order, the heading that
/// repeats the title decided left out, and led by the page's description
/// and image when the page shows them apart from the article.
pub(super) fn article<'a>(
    page: &Page<'a>,
    lead: Lead,
    decisions: &mut Decisions,
) -> Vec<NodeRef<'a>> {
    let hints = weights(page, HINTS, decisions);
    let removed = decisions.removed(page);
    let document = page.document;
    let mut scores: HashMap<NodeId, (NodeRef, f64)> = HashMap::new();
    let paragraphs = document
        .select("p, pre, td, blockquote")
        .nodes()
        .iter()
        .copied()
        .chain(
            // A `div` written as a paragraph, holding only text and inline
            // elements, or holding its paragraphs as bare text between
            // breaks.
            document
                .select("div, section")
                .nodes()
                .iter()
                .filter(|block| {
                    (!page.holds_block(block) && is_prose(page, block))
                        || bare_text(page, block) >= BARE_TEXT_MIN
                })
                .copied(),
        )
        .collect::<Vec<_>>();
    for paragraph in paragraphs {
        let size = page.text(&paragraph);
        if size < PARAGRAPH_MIN || removed.contains(&paragraph.id) {
            continue;
        }
        let text = paragraph.text();
        let commas = text.matches([',', '，']).count() as f64;
        let score = 1.0 + commas + (size / 100).min(3) as f64;
        for (level, ancestor) in std::iter::once(paragraph)
            .chain(paragraph.ancestors_it(None))
            .skip(1)
            .take(LEVELS)
            .enumerate()
        {
            if !ancestor.is_element() || ancestor.has_name("html") {
                break;
            }
            let divider = match level {
                0 => 1.0,
                1 => 2.0,
                level => level as f64 * 3.0,
            };
            scores
                .entry(ancestor.id)
                .or_insert_with(|| (ancestor, initial(&ancestor)))
                .1 += score / divider;
        }
    }
    // A name's hint scales what the text earned, so a name alone never
    // makes an element the article.
    let scored = |node: &NodeRef| {
        let hint = hints.get(&node.id).copied().unwrap_or_default();
        let factor = (1.0 + hint / HINT_SCALE).max(0.1);
        scores
            .get(&node.id)
            .map(|(_, score)| score * factor * (1.0 - page.link_density(node)))
    };
    let mut ranked = scores
        .values()
        .filter_map(|(node, _)| Some((*node, scored(node)?)))
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let best = ranked.first().copied().map(|(top, score)| {
        let top = shared_ancestor(page, top, score, &ranked).unwrap_or(top);
        climbed(top, &scored)
    });
    let mut roots = match best {
        None => document.select("body").nodes().to_vec(),
        Some((best, score)) => {
            let threshold = (score * 0.2).max(SIBLING_MIN);
            let class = best.attr_or("class", "");
            let siblings = match best.parent() {
                Some(parent) if !parent.has_name("html") => parent.element_children(),
                _ => vec![best],
            };
            siblings
                .into_iter()
                .filter(|sibling| {
                    sibling.id == best.id
                        || scored(sibling).is_some_and(|score| score >= threshold)
                        || (!class.is_empty()
                            && sibling.attr_or("class", "") == class
                            && page.text(sibling) > 0
                            && page.link_density(sibling) < 0.5)
                        || is_more_text(page, sibling)
                        || is_picture(sibling)
                        || (is_image(page, sibling)
                            && best
                                .prev_element_sibling()
                                .is_some_and(|before| before.id == sibling.id))
                })
                .collect()
        }
    };
    if let Some(image) = lead
        .image
        .and_then(|image| lead_image(page, image, &roots, decisions, &removed))
    {
        roots.insert(0, image);
    }
    if let Some(description) = lead
        .descriptions
        .iter()
        .find_map(|description| lead_description(page, description, &roots, &removed))
    {
        roots.insert(0, description);
    }
    if let Some(title) = lead.title {
        let title = collapsed(title).to_lowercase();
        let heading = roots
            .iter()
            .flat_map(|root| std::iter::once(*root).chain(root.descendants_it()))
            .find(|node| {
                (node.has_name("h1") || node.has_name("h2"))
                    && collapsed(&node.text()).to_lowercase() == title
            });
        if let Some(heading) = heading {
            decisions.delete(&heading);
        }
    }
    roots
}

/// The page's lead image, outside the article's `roots`: the first image
/// no rule left out showing the file `image` names, or the figure holding
/// it, unless the article already shows that file.
fn lead_image<'a>(
    page: &Page<'a>,
    image: &str,
    roots: &[NodeRef<'a>],
    decisions: &Decisions,
    removed: &HashSet<NodeId>,
) -> Option<NodeRef<'a>> {
    let file = |address: &str| {
        let path = address.split(['?', '#']).next().unwrap_or_default();
        path.rsplit('/').next().unwrap_or_default().to_string()
    };
    let wanted = file(image);
    if wanted.is_empty() {
        return None;
    }
    let shows = |node: &NodeRef| node.has_name("img") && file(&decisions.source(node)) == wanted;
    let in_article = roots
        .iter()
        .flat_map(|root| std::iter::once(*root).chain(root.descendants_it()))
        .any(|node| shows(&node));
    if in_article {
        return None;
    }
    let lead = page
        .document
        .select("img")
        .nodes()
        .iter()
        .find(|node| shows(node) && !removed.contains(&node.id))
        .copied()?;
    Some(
        lead.ancestors_it(None)
            .find(|ancestor| ancestor.has_name("figure"))
            .unwrap_or(lead),
    )
}

/// The element showing the page's `description`, outside the article's
/// `roots`, such as a subtitle under the headline, unless the article
/// already says it. A description only the page's metadata holds is left
/// there.
fn lead_description<'a>(
    page: &Page<'a>,
    description: &str,
    roots: &[NodeRef<'a>],
    removed: &HashSet<NodeId>,
) -> Option<NodeRef<'a>> {
    let wanted = collapsed(description);
    if wanted.len() < PARAGRAPH_MIN {
        return None;
    }
    // Text that collapses alike holds as many bytes besides whitespace.
    let size: usize = wanted
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(char::len_utf8)
        .sum();
    let said = roots
        .iter()
        .any(|root| collapsed(&root.text()).contains(&wanted));
    if said {
        return None;
    }
    page.document
        .select("body p, body h2, body div, body span")
        .nodes()
        .iter()
        .find(|node| {
            page.text(node) == size
                && collapsed(&node.text()) == wanted
                && !removed.contains(&node.id)
                && !roots.iter().any(|root| holds(root, node))
        })
        .copied()
}

/// What `element` scores before its paragraphs count: what its tag says
/// of it.
fn initial(element: &NodeRef) -> f64 {
    let name = element.node_name().unwrap_or_default().to_ascii_lowercase();
    match name.as_str() {
        "div" | "article" | "main" | "section" => 5.0,
        "pre" | "td" | "blockquote" => 3.0,
        "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" => -3.0,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" => -5.0,
        _ => 0.0,
    }
}

/// Whether `block`'s text reads as prose rather than a label or a
/// headline: long, or holding the end of a sentence.
fn is_prose(page: &Page, block: &NodeRef) -> bool {
    if page.text(block) >= BARE_TEXT_MIN {
        return true;
    }
    let text = block.text();
    text.split_whitespace()
        .any(|word| word.len() > 1 && word.ends_with(['.', '!', '?', '。']))
}

/// How much text `element` holds directly, outside its child elements.
fn bare_text(page: &Page, element: &NodeRef) -> usize {
    element
        .children_it(false)
        .filter(|child| child.is_text())
        .map(|text| page.text(&text))
        .sum()
}

/// The ancestor that `top` and at least two more of the best scored
/// elements close to it in score share, and whose text they make up most
/// of: the article's container, when its parts score alike, such as the
/// entries of a page of release notes.
fn shared_ancestor<'a>(
    page: &Page,
    top: NodeRef<'a>,
    score: f64,
    ranked: &[(NodeRef<'a>, f64)],
) -> Option<NodeRef<'a>> {
    let rivals: Vec<NodeRef> = ranked
        .iter()
        .skip(1)
        .take(RIVALS - 1)
        .filter(|(_, rival)| *rival >= score * 0.75)
        .map(|(node, _)| *node)
        .collect();
    if rivals.len() < 2 {
        return None;
    }
    let ancestor = top
        .ancestors_it(None)
        .take_while(|ancestor| !(ancestor.has_name("html") || ancestor.has_name("body")))
        .find(|ancestor| rivals.iter().filter(|rival| holds(ancestor, rival)).count() >= 2)?;
    // It is made of them, not a page holding them among much else.
    let united: usize = std::iter::once(top)
        .chain(rivals.into_iter().filter(|rival| holds(&ancestor, rival)))
        .map(|node| page.text(&node))
        .sum();
    (united * 2 >= page.text(&ancestor)).then_some(ancestor)
}

/// Whether `inner` is within `outer`.
fn holds(outer: &NodeRef, inner: &NodeRef) -> bool {
    inner.ancestors_it(None).any(|node| node.id == outer.id)
}

/// `top`, or the ancestor it climbs to: one scoring higher than what it
/// holds, while each step out keeps at least a third of `top`'s score, and
/// no further out than the `<article>` holding it.
fn climbed<'a>(top: NodeRef<'a>, scored: &dyn Fn(&NodeRef) -> Option<f64>) -> (NodeRef<'a>, f64) {
    let start = scored(&top).unwrap_or_default();
    let threshold = start / 3.0;
    let mut last = start;
    for ancestor in top.ancestors_it(None) {
        if ancestor.has_name("html") || ancestor.has_name("body") {
            break;
        }
        if let Some(score) = scored(&ancestor) {
            if score < threshold {
                break;
            }
            if score > last {
                return (ancestor, score);
            }
            last = score;
        }
        if ancestor.has_name("article") {
            break;
        }
    }
    (top, start)
}

/// Whether `sibling` of the article's element reads as more of its text: a
/// paragraph of prose rather than links.
fn is_more_text(page: &Page, sibling: &NodeRef) -> bool {
    if !sibling.has_name("p") {
        return false;
    }
    let size = page.text(sibling);
    let density = page.link_density(sibling);
    (size > 80 && density < 0.25)
        || (size > 0 && density == 0.0 && sibling.text().trim_end().ends_with('.'))
}

/// Whether `sibling` of the article's element is a picture with its
/// caption.
fn is_picture(sibling: &NodeRef) -> bool {
    sibling.has_name("figure") || sibling.has_name("picture")
}

/// Whether `element` holds an image and no text: the article's lead image,
/// when it comes just before the article.
fn is_image(page: &Page, element: &NodeRef) -> bool {
    page.text(element) == 0 && page.holds_image(element)
}
