//! Coherence's Markdown writer: an article's HTML as the Markdown a reader,
//! an LLM, a search excerpt or a plain-text preview reads, in one walk that
//! carries out what the rules decided as it meets each node. It escapes a
//! character only where Markdown would give it a meaning, so prose reads
//! `Linux 7.4 (beta)!` rather than `Linux 7\.4 \(beta\)\!`, makes every
//! link and image absolute, and leaves out what a reader of the page never
//! sees as text: scripts, controls, a link's tooltip. Spaces at the edges
//! of bold or italic text move outside it, as `**` can't open before a
//! space or close after one.

use dom_query::{Document, NodeRef};

use super::rule::{Decisions, Embed, Fate};
use crate::article_snapshot::Url;

/// What `roots` hold, in order, as Markdown, as `decisions` have it; links
/// and images made absolute against `base`.
pub(super) fn markdown(roots: &[NodeRef], decisions: &Decisions, base: &Url) -> String {
    let mut writer = Writer::new(Context::Flow, decisions, base);
    for root in roots {
        writer.node(root);
    }
    writer.finish()
}

/// Plain `text` as Markdown paragraphs, one for each run of it between
/// blank lines.
pub(super) fn paragraphs(text: &str) -> String {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| escape(&paragraph, None, true, false, false))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Where a run of inline text is being written.
#[derive(Clone, Copy, PartialEq)]
enum Context {
    /// A paragraph, heading or list item: blocks break it.
    Flow,
    /// A link's text or a table's cell: on one line, its blocks run
    /// together.
    Line { link: bool, cell: bool },
}

/// Markdown written so far: whole blocks, and the paragraph being written.
struct Writer<'a> {
    context: Context,
    decisions: &'a Decisions,
    base: &'a Url,
    blocks: Vec<String>,
    /// The paragraph being written. A hard line break is a bare `\n`, which
    /// collapsed text never holds.
    line: String,
}

/// Elements whose contents are never text a reader sees.
const UNSEEN: &[&str] = &[
    "head", "title", "meta", "link", "script", "style", "noscript", "template", "svg", "math",
    "button", "input", "select", "option", "textarea", "iframe", "object", "embed", "canvas",
    "audio", "video", "source", "track", "map", "area", "dialog",
];

/// Elements that stand on their own, apart from the text around them.
const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "body",
    "center",
    "details",
    "div",
    "dl",
    "dd",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "header",
    "hgroup",
    "html",
    "legend",
    "main",
    "nav",
    "p",
    "section",
    "summary",
];

impl<'a> Writer<'a> {
    fn new(context: Context, decisions: &'a Decisions, base: &'a Url) -> Self {
        Self {
            context,
            decisions,
            base,
            blocks: Vec::new(),
            line: String::new(),
        }
    }

    /// A writer for what `self` holds, in `context`.
    fn nested(&self, context: Context) -> Self {
        Self::new(context, self.decisions, self.base)
    }

    /// `address` made absolute.
    fn absolute(&self, address: &str) -> String {
        self.base
            .join(address)
            .map_or_else(|_| address.to_string(), |url| url.to_string())
    }

    fn children(&mut self, node: &NodeRef) {
        for child in node.children_it(false) {
            self.node(&child);
        }
    }

    fn node(&mut self, node: &NodeRef) {
        match self.decisions.fate(node) {
            Some(Fate::Deleted) => return,
            Some(Fate::Unwrapped) => {
                self.children(node);
                return;
            }
            Some(Fate::LinkedTo(embed)) => {
                self.embed(node, embed);
                return;
            }
            None => {}
        }
        if node.is_text() {
            self.text(&node.text());
            return;
        }
        if node.is_document() || node.is_fragment() {
            self.children(node);
            return;
        }
        if !node.is_element() {
            return;
        }
        let name = node.node_name().unwrap_or_default().to_ascii_lowercase();
        let name = name.as_str();
        if UNSEEN.contains(&name) {
            return;
        }
        if let Context::Line { .. } = self.context
            && (BLOCKS.contains(&name)
                || matches!(
                    name,
                    "h1" | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "ul"
                        | "ol"
                        | "li"
                        | "blockquote"
                        | "pre"
                        | "table"
                        | "tr"
                        | "td"
                        | "th"
                        | "hr"
                ))
        {
            self.space();
            self.children(node);
            self.space();
            return;
        }
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.heading(node, name),
            "ul" => self.list(node, None),
            "ol" => {
                let start = node
                    .attr("start")
                    .and_then(|start| start.trim().parse().ok())
                    .unwrap_or(1);
                self.list(node, Some(start))
            }
            "blockquote" => self.quote(node),
            "pre" => self.code_block(node),
            "table" => self.table(node),
            "hr" => {
                self.flush();
                self.blocks.push("---".to_string());
            }
            "br" => {
                self.line.truncate(self.line.trim_end_matches(' ').len());
                // Two breaks in a row part paragraphs.
                if self.context == Context::Flow && self.line.ends_with('\n') {
                    self.flush();
                } else {
                    self.line.push('\n');
                }
            }
            "img" => self.image(node),
            "a" => self.link(node),
            "strong" | "b" => self.emphasis(node, "**"),
            "em" | "i" => self.emphasis(node, "*"),
            "del" | "s" | "strike" => self.emphasis(node, "~~"),
            "code" | "kbd" | "samp" | "tt" => self.code_span(node),
            "q" => {
                self.line.push('“');
                self.children(node);
                self.line.push('”');
            }
            _ if BLOCKS.contains(&name) => {
                self.flush();
                self.children(node);
                self.flush();
            }
            // Anything else, a `span` or an element of a site's own, is
            // what it holds.
            _ => self.children(node),
        }
    }

    /// Ends the paragraph being written. A line that holds only whitespace
    /// a reader can't see, such as a no-break space after a `<br>`, parts
    /// paragraphs as two breaks in a row do: a hard break needs text after
    /// it, or its backslash shows.
    fn flush(&mut self) {
        let lines: Vec<&str> = self.line.split('\n').map(str::trim).collect();
        for paragraph in lines.split(|line| line.is_empty()) {
            if !paragraph.is_empty() {
                self.blocks.push(paragraph.join("\\\n"));
            }
        }
        self.line.clear();
    }

    /// The blocks written, apart by blank lines.
    fn finish(mut self) -> String {
        self.flush();
        self.blocks.join("\n\n")
    }

    /// What `node` holds, written on one line.
    fn inline(&self, node: &NodeRef, link: bool) -> String {
        let (was_link, cell) = match self.context {
            Context::Line { link, cell } => (link, cell),
            Context::Flow => (false, false),
        };
        let mut writer = self.nested(Context::Line {
            link: link || was_link,
            cell,
        });
        writer.children(node);
        let mut text = writer.blocks.join(" ");
        if !text.is_empty() && !writer.line.is_empty() {
            text.push(' ');
        }
        text.push_str(&writer.line);
        text
    }

    /// A space, unless the line is empty or already ends in one.
    fn space(&mut self) {
        if !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
            self.line.push(' ');
        }
    }

    /// Whether what is written next opens a line, where a block's marker
    /// would start.
    fn at_line_start(&self) -> bool {
        self.context == Context::Flow
            && self
                .line
                .rsplit('\n')
                .next()
                .is_none_or(|last| last.trim().is_empty())
    }

    /// Text, its whitespace collapsed and escaped where it stands.
    fn text(&mut self, text: &str) {
        let mut collapsed = String::with_capacity(text.len());
        for c in text.chars() {
            if c.is_ascii_whitespace() {
                if !collapsed.ends_with(' ') {
                    collapsed.push(' ');
                }
            } else {
                collapsed.push(c);
            }
        }
        if self.line.is_empty() || self.line.ends_with([' ', '\n']) {
            collapsed = collapsed.trim_start_matches(' ').to_string();
        }
        if collapsed.is_empty() {
            return;
        }
        let line_start = self.at_line_start();
        let before = self.line.chars().last();
        let (link, cell) = match self.context {
            Context::Line { link, cell } => (link, cell),
            Context::Flow => (false, false),
        };
        self.line
            .push_str(&escape(&collapsed, before, line_start, link, cell));
    }

    fn heading(&mut self, node: &NodeRef, name: &str) {
        let text = self.inline(node, false).replace('\n', " ");
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.flush();
        // The article's title shows above it, so its own headings start a
        // level below.
        let level = name[1..].parse::<usize>().unwrap_or(1).max(2);
        // Closing hashes would read as the heading's own.
        let text = match text.strip_suffix('#') {
            Some(rest) => format!("{rest}\\#"),
            None => text.to_string(),
        };
        self.blocks.push(format!("{} {text}", "#".repeat(level)));
    }

    /// A list, numbered from `start` when it is ordered.
    fn list(&mut self, node: &NodeRef, start: Option<u64>) {
        self.flush();
        let mut items: Vec<String> = Vec::new();
        let mut number = start.unwrap_or(1);
        for child in node.children_it(false) {
            if !child.is_element() {
                continue;
            }
            let mut writer = self.nested(self.context);
            if child.has_name("li") {
                writer.children(&child);
            } else {
                // A list nested in a list without an item of its own.
                writer.node(&child);
            }
            writer.flush();
            // A list nested in an item follows its text with no blank line,
            // keeping the outer list tight.
            let mut item = String::new();
            for block in &writer.blocks {
                if !item.is_empty() {
                    item.push_str(if is_list(block) { "\n" } else { "\n\n" });
                }
                item.push_str(block);
            }
            if item.is_empty() {
                continue;
            }
            if !child.has_name("li")
                && let Some(last) = items.last_mut()
            {
                last.push('\n');
                last.push_str(&indented(&item, "  "));
                continue;
            }
            let marker = match start {
                Some(_) => format!("{number}. "),
                None => "- ".to_string(),
            };
            number += 1;
            let indent = " ".repeat(marker.len());
            items.push(format!("{marker}{}", indented(&item, &indent).trim_start()));
        }
        if !items.is_empty() {
            self.blocks.push(items.join("\n"));
        }
    }

    fn quote(&mut self, node: &NodeRef) {
        self.flush();
        let mut writer = self.nested(self.context);
        writer.children(node);
        let quoted = writer.finish();
        if quoted.is_empty() {
            return;
        }
        let quoted = quoted
            .lines()
            .map(|line| match line {
                "" => ">".to_string(),
                line => format!("> {line}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.blocks.push(quoted);
    }

    fn code_block(&mut self, node: &NodeRef) {
        let text = node.text();
        let code = text.strip_prefix('\n').unwrap_or(&text).trim_end();
        if code.trim().is_empty() {
            return;
        }
        self.flush();
        let language = std::iter::once(*node)
            .chain(node.descendants_it().filter(|inner| inner.has_name("code")))
            .flat_map(|element| {
                element
                    .attr_or("class", "")
                    .split_whitespace()
                    .find_map(|class| {
                        class
                            .strip_prefix("language-")
                            .or_else(|| class.strip_prefix("lang-"))
                            .map(str::to_string)
                    })
            })
            .next()
            .unwrap_or_default();
        let fence = "`".repeat(longest_run(code, '`').max(2) + 1);
        self.blocks
            .push(format!("{fence}{language}\n{code}\n{fence}"));
    }

    fn table(&mut self, table: &NodeRef) {
        let rows: Vec<NodeRef> = table
            .descendants_it()
            .filter(|node| {
                node.has_name("tr")
                    && node
                        .ancestors_it(None)
                        .find(|ancestor| ancestor.has_name("table"))
                        .is_some_and(|owner| owner.id == table.id)
            })
            .collect();
        let nested = table.descendants_it().any(|node| node.has_name("table"));
        let width = rows
            .iter()
            .map(|row| row.element_children().len())
            .max()
            .unwrap_or(0);
        // A table laying out a page, or of one cell, is what it holds.
        if nested || width < 2 {
            self.flush();
            self.children(table);
            self.flush();
            return;
        }
        let cell_writer = self.nested(Context::Line {
            link: false,
            cell: true,
        });
        let mut lines = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let mut cells: Vec<String> = row
                .element_children()
                .iter()
                .filter(|cell| cell.is("td, th"))
                .map(|cell| {
                    let text = cell_writer.inline(cell, false).replace('\n', " ");
                    text.trim().to_string()
                })
                .collect();
            cells.resize(width, String::new());
            lines.push(format!("| {} |", cells.join(" | ")));
            if index == 0 {
                lines.push(format!("|{}", " --- |".repeat(width)));
            }
        }
        self.flush();
        self.blocks.push(lines.join("\n"));
    }

    fn image(&mut self, image: &NodeRef) {
        let source = self.decisions.source(image);
        if source.is_empty() {
            return;
        }
        let source = self.absolute(&source);
        let alt = image.attr_or("alt", "");
        // A site's alt written as HTML (`&lt;em&gt;A caption&lt;/em&gt;`).
        let alt = if alt.contains(['<', '&']) {
            Document::fragment(alt).root().text()
        } else {
            alt
        };
        let mut plain = String::with_capacity(alt.len());
        for word in alt.split_whitespace() {
            if !plain.is_empty() {
                plain.push(' ');
            }
            for c in word.chars() {
                if matches!(c, '[' | ']' | '\\') {
                    plain.push('\\');
                }
                plain.push(c);
            }
        }
        self.line
            .push_str(&format!("![{plain}]({})", destination(&source)));
    }

    fn link(&mut self, link: &NodeRef) {
        let text = self.inline(link, true);
        let href = link
            .attr("href")
            .map(|href| href.trim().to_string())
            .filter(|href| !href.is_empty() && !href.starts_with("javascript:"))
            .map(|href| self.absolute(&href));
        let in_link = matches!(self.context, Context::Line { link: true, .. });
        let Some(href) = href.filter(|_| !in_link) else {
            self.push_inline(&text);
            return;
        };
        let (lead, core, trail) = edges(&text, link);
        if core.is_empty() {
            self.push_inline(lead);
            return;
        }
        self.push_inline(lead);
        self.line
            .push_str(&format!("[{core}]({})", destination(&href)));
        self.line.push_str(trail);
    }

    /// A paragraph linking where `node`, an embed, leads in its place.
    fn embed(&mut self, node: &NodeRef, embed: Embed) {
        let Some((address, shown)) = embed.link(node) else {
            return;
        };
        self.flush();
        let text = escape(&shown, None, true, true, false);
        self.blocks
            .push(format!("[{text}]({})", destination(address.as_str())));
    }

    /// Bold, italic or struck text, its edges' spaces moved outside it.
    fn emphasis(&mut self, node: &NodeRef, marker: &str) {
        let text = self.inline(node, false);
        let (lead, core, trail) = edges(&text, node);
        self.push_inline(lead);
        if !core.is_empty() {
            // Nested emphasis of the same kind, `<b><strong>`, is one.
            let marked =
                core.starts_with(marker) && core.ends_with(marker) && core.len() > 2 * marker.len();
            if marked {
                self.line.push_str(core);
            } else {
                self.line.push_str(&format!("{marker}{core}{marker}"));
            }
        }
        self.push_inline(trail);
    }

    fn code_span(&mut self, node: &NodeRef) {
        let code = node.text().split_whitespace().collect::<Vec<_>>().join(" ");
        if code.is_empty() {
            return;
        }
        let fence = "`".repeat(longest_run(&code, '`') + 1);
        let pad = if code.starts_with('`') || code.ends_with('`') {
            " "
        } else {
            ""
        };
        self.line
            .push_str(&format!("{fence}{pad}{code}{pad}{fence}"));
    }

    /// Already written inline Markdown, its leading space dropped where the
    /// line has one or is empty.
    fn push_inline(&mut self, text: &str) {
        if self.line.is_empty() || self.line.ends_with([' ', '\n']) {
            self.line.push_str(text.trim_start_matches(' '));
        } else {
            self.line.push_str(text);
        }
    }
}

/// `text`, written from `node`, split into a space for the whitespace
/// `node` opens with, what it surrounds, and a space for the whitespace it
/// ends with. Written text has its opening whitespace trimmed, so `node`'s
/// own text says whether there was any.
fn edges<'t>(text: &'t str, node: &NodeRef) -> (&'static str, &'t str, &'static str) {
    let core = text.trim_matches(|c: char| c == ' ' || c == '\n');
    let source = node.text();
    let lead = if source.starts_with(char::is_whitespace) || text.starts_with(' ') {
        " "
    } else {
        ""
    };
    let trail = if source.ends_with(char::is_whitespace) || text.ends_with(' ') {
        " "
    } else {
        ""
    };
    (lead, core, trail)
}

/// Whether `block` is a list. Text never opens with a list's marker: its
/// own would be escaped.
fn is_list(block: &str) -> bool {
    let digits = block.chars().take_while(char::is_ascii_digit).count();
    block.starts_with("- ") || (digits > 0 && block[digits..].starts_with(". "))
}

/// `text` indented by `indent`, but on its blank lines.
fn indented(text: &str, indent: &str) -> String {
    text.lines()
        .map(|line| match line {
            "" => String::new(),
            line => format!("{indent}{line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The longest run of `c` in `text`.
fn longest_run(text: &str, c: char) -> usize {
    text.split(|other| other != c)
        .map(str::len)
        .max()
        .unwrap_or(0)
}

/// `address` as a link's or image's destination: in angle brackets when a
/// space or unbalanced parenthesis would end it early.
fn destination(address: &str) -> String {
    let mut depth = 0i32;
    let mut balanced = true;
    for c in address.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                balanced &= depth >= 0;
            }
            _ => {}
        }
    }
    if balanced && depth == 0 && !address.contains([' ', '<', '>']) {
        address.to_string()
    } else {
        format!("<{}>", address.replace('<', "%3C").replace('>', "%3E"))
    }
}

/// `text`, collapsed prose written after `before` on its line, with a
/// backslash before each character Markdown would otherwise read as markup
/// there. `line_start` when it opens a line, where a block's marker would
/// start; `link` inside a link's text, where a bracket would end it;
/// `cell` inside a table's cell, where a bar would.
fn escape(text: &str, before: Option<char>, line_start: bool, link: bool, cell: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let marker_end = if line_start {
        block_marker_end(&chars)
    } else {
        None
    };
    for (i, &c) in chars.iter().enumerate() {
        let previous = if i == 0 { before } else { Some(chars[i - 1]) };
        let next = chars.get(i + 1).copied();
        let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
        let alphanumeric = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
        let needed = Some(i) == marker_end
            || match c {
                '\\' => next.is_some_and(|next| next.is_ascii_punctuation()),
                '`' => true,
                // Emphasis can't open or close between spaces.
                '*' => !(space(previous) && space(next)),
                // Nor can an underscore inside a word.
                '_' => {
                    !(alphanumeric(previous) && alphanumeric(next)
                        || space(previous) && space(next))
                }
                '~' => previous == Some('~') || next == Some('~'),
                '[' | ']' if link => true,
                ']' => matches!(next, Some('(' | '[' | ':')),
                '!' => next == Some('['),
                '<' => next.is_some_and(|next| next.is_ascii_alphabetic() || "/!?".contains(next)),
                '&' => is_entity(&chars[i + 1..]),
                '|' => cell,
                _ => false,
            };
        if needed {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Where, in a line's text, the character that would make it a heading,
/// list item, quote or rule sits, when it would.
fn block_marker_end(chars: &[char]) -> Option<usize> {
    let after = |i: usize| chars.get(i).is_none_or(|c| c.is_whitespace());
    match chars.first()? {
        '#' => {
            let hashes = chars.iter().take_while(|&&c| c == '#').count();
            (hashes <= 6 && after(hashes)).then_some(0)
        }
        '>' => Some(0),
        '-' | '+' | '*' | '=' if after(1) || chars.get(1) == chars.first() => Some(0),
        c if c.is_ascii_digit() => {
            let digits = chars.iter().take_while(|c| c.is_ascii_digit()).count();
            (digits <= 9 && matches!(chars.get(digits), Some('.' | ')')) && after(digits + 1))
                .then_some(digits)
        }
        _ => None,
    }
}

/// Whether `rest`, after an `&`, would read as an HTML entity.
fn is_entity(rest: &[char]) -> bool {
    let rest = rest.strip_prefix(&['#']).unwrap_or(rest);
    let name = rest
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric())
        .count();
    name > 0 && rest.get(name) == Some(&';')
}

#[cfg(test)]
mod tests;
