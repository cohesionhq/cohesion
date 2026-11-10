// `cohesion://` links to items, read and written as the daemon does
// (`ItemLink`, `TextPiece` in cohesion-core): what a link names, and where a
// text shows items as cards.

/** What a `cohesion://` link can point at. */
export type ItemKind = WorkspaceItemKind | 'database-row' | 'agent' | 'actor-session' | 'article';

/** What a `cohesion://` link names with its workspace. */
export type WorkspaceItemKind = 'task' | 'document' | 'resource' | 'database' | 'contact';

/** What a `cohesion://` link can name without its workspace. */
export type UnscopedItemKind = 'task' | 'document' | 'agent' | 'actor-session' | 'article';

// `cohesion://task/3/42`, `cohesion://document/3/7`, `cohesion://resource/3/9`,
// `cohesion://database/3/2` and `cohesion://contact/3/12` (workspace, then item); a database row's
// `cohesion://database-row/3/2/14` (workspace, database, then row);
// `cohesion://agent/3`, `cohesion://actor-session/143` and
// `cohesion://article/88`. The daemon reads them the same way (`ItemLink`).
const WORKSPACE_ITEM_LINK =
	/^cohesion:\/\/(task|document|resource|database|contact)\/(\d+)\/(\d+)$/;
const DATABASE_ROW_LINK = /^cohesion:\/\/database-row\/(\d+)\/(\d+)\/(\d+)$/;
// `cohesion://task/42` and `cohesion://document/7` without their workspace,
// as links were first written, as well as agents, actor sessions and articles.
const ITEM_LINK = /^cohesion:\/\/(task|document|agent|actor-session|article)\/(\d+)$/;

/**
 * What a `cohesion://` link names: a task, document, resource, database or
 * contact with its workspace (`cohesion://task/3/42`), a database row with its
 * workspace and database (`cohesion://database-row/3/2/14`), or an item
 * without one (`cohesion://task/42`, `cohesion://agent/3`), as links to tasks
 * and documents were first written.
 */
export type ItemLink =
	| { kind: WorkspaceItemKind; ws: number; id: number }
	| { kind: 'database-row'; ws: number; database: number; id: number }
	| { kind: UnscopedItemKind; ws: null; id: number };

/** What a link to an item (`cohesion://task/3/42`) names, or null for any other href. */
export function parseItemLink(href: string): ItemLink | null {
	const scoped = WORKSPACE_ITEM_LINK.exec(href);
	if (scoped)
		return {
			kind: scoped[1] as WorkspaceItemKind,
			ws: Number(scoped[2]),
			id: Number(scoped[3])
		};
	const row = DATABASE_ROW_LINK.exec(href);
	if (row)
		return {
			kind: 'database-row',
			ws: Number(row[1]),
			database: Number(row[2]),
			id: Number(row[3])
		};
	const match = ITEM_LINK.exec(href);
	return match ? { kind: match[1] as UnscopedItemKind, ws: null, id: Number(match[2]) } : null;
}

/** `link` as written: `cohesion://task/3/42`. */
export function itemLinkHref(link: ItemLink): string {
	if (link.kind === 'database-row')
		return `cohesion://database-row/${link.ws}/${link.database}/${link.id}`;
	return link.ws === null
		? `cohesion://${link.kind}/${link.id}`
		: `cohesion://${link.kind}/${link.ws}/${link.id}`;
}

/**
 * `[label](href)`, with the brackets and backslashes in `label` kept as text:
 * the escape `embedLine` reads back, so the link stands alone on a line as one.
 */
export function markdownLink(label: string, href: string): string {
	return `[${label.replace(/[[\]\\]/g, (c) => `\\${c}`)}](${href})`;
}

/**
 * `link` as a Markdown link titled `title`, as picking the item after `@`
 * writes it. A title spans one line, so its line breaks and runs of spaces
 * become one space.
 */
export function itemLinkMarkdown(link: ItemLink, title: string): string {
	return markdownLink(title.replace(/\s+/g, ' ').trim(), itemLinkHref(link));
}

/** A piece of Markdown text, split as `text_pieces` in cohesion-core splits it. */
export type TextPiece =
	| { kind: 'markdown'; text: string }
	| { kind: 'embed'; link: ItemLink; href: string; label: string };

const FENCE = /^\s*(```|~~~)/;
const ASCII_PUNCTUATION = /^[!-/:-@[-`{-~]$/;

/**
 * The label, href and link of `line` when it is exactly one link to an item
 * (spaces around it aside, but not a code block's indent of four), or null.
 * The label ends at its first unescaped `]`, which must be followed by `(`,
 * and the link at the line's last `)`: so `[A](https://a.dev) and
 * [B](cohesion://…)`, whose first link isn't an item's, and two links on one
 * line are not embeds. A `\` before punctuation in the label is Markdown's
 * escape and is dropped.
 *
 * `embed_line` in cohesion-core (model/text_section.rs) reads a line the same
 * way; change them together.
 */
export function embedLine(line: string): { label: string; href: string; link: ItemLink } | null {
	const indent = line.slice(0, line.length - line.trimStart().length);
	if (indent.includes('\t') || indent.length >= 4) return null;
	const trimmed = line.trim();
	if (!trimmed.startsWith('[')) return null;
	let label = '';
	let close = -1;
	for (let i = 1; i < trimmed.length; i++) {
		const c = trimmed[i];
		if (c === '\\') {
			const next = trimmed[++i];
			if (next === undefined) return null;
			label += ASCII_PUNCTUATION.test(next) ? next : c + next;
		} else if (c === ']') {
			close = i;
			break;
		} else label += c;
	}
	if (close < 0 || trimmed[close + 1] !== '(' || !trimmed.endsWith(')')) return null;
	const href = trimmed.slice(close + 2, -1);
	const link = parseItemLink(href);
	return link && { label, href, link };
}

/**
 * `text` split into its Markdown and its embeds: each line that is only a
 * `[label](cohesion://…)` link to an item, outside a fenced or indented code
 * block, is an embed. Blank lines around an embed go with it, and Markdown of
 * only blank lines is dropped. The daemon's `resolveEmbeds` splits the same way.
 */
export function textPieces(text: string): TextPiece[] {
	const pieces: TextPiece[] = [];
	let markdown: string[] = [];
	let fenced = false;
	const flush = () => {
		const start = markdown.findIndex((l) => l.trim() !== '');
		const end = markdown.findLastIndex((l) => l.trim() !== '');
		if (start >= 0)
			pieces.push({ kind: 'markdown', text: markdown.slice(start, end + 1).join('\n') });
		markdown = [];
	};
	for (const line of text.split(/\r?\n/)) {
		if (FENCE.test(line)) fenced = !fenced;
		const embed = fenced ? null : embedLine(line);
		if (embed) {
			flush();
			pieces.push({ kind: 'embed', ...embed });
		} else markdown.push(line);
	}
	flush();
	return pieces;
}
