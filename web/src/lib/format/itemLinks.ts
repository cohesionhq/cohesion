// `cohesion://` links to items, read and written as the daemon does
// (`ItemLink`, `TextPiece` in cohesion-core): what a link names, and where a
// text shows items as cards.

import type { Invalidation, LinkKind } from '$lib/protocol/types';

/** What the app knows of one kind of item a `cohesion://` link can name. */
interface ItemKindInfo {
	/** How the daemon names the kind, in an embed and in `backlinks`. */
	wire: LinkKind;
	/** Whether a link names it with its workspace: `cohesion://task/3/42`. */
	scoped: boolean;
	/** Whether a link names it without one: `cohesion://task/42`, `cohesion://agent/3`. */
	unscoped: boolean;
	/** The invalidation that says an item of the kind changed somewhere. */
	changes: Invalidation['kind'];
	/** What an item of the kind is called where one isn't found: "Task not found". */
	noun: string;
}

/**
 * Every kind of item a `cohesion://` link can name, by the word the link
 * uses: `cohesion://task/3/42`, `cohesion://document/3/7`,
 * `cohesion://resource/3/9`, `cohesion://document-folder/3/2` and
 * `cohesion://contact/3/12` (workspace, then item; a folder is how a table
 * is linked); `cohesion://agent/3`, `cohesion://actor-session/143` and
 * `cohesion://article/88`; and `cohesion://task/42` and
 * `cohesion://document/7` without their workspace, as links were first
 * written. The daemon reads them the same way (`ItemLink`).
 *
 * A new kind is one entry here, its icon (`shell/embedIcons.ts`), and what
 * the compiler then asks for: where its embed opens (`shell/links.ts`), its
 * panel (`shell/linkedItem.svelte.ts`) and its card's detail
 * (`shell/EmbedCard.svelte`).
 */
export const ITEM_KINDS = {
	task: { wire: 'task', scoped: true, unscoped: true, changes: 'taskList', noun: 'Task' },
	document: {
		wire: 'document',
		scoped: true,
		unscoped: true,
		changes: 'documents',
		noun: 'Document'
	},
	resource: {
		wire: 'resource',
		scoped: true,
		unscoped: false,
		changes: 'resources',
		noun: 'Resource'
	},
	'document-folder': {
		wire: 'documentFolder',
		scoped: true,
		unscoped: false,
		changes: 'documentFolders',
		noun: 'Folder'
	},
	contact: {
		wire: 'contact',
		scoped: true,
		unscoped: false,
		changes: 'contacts',
		noun: 'Contact'
	},
	agent: { wire: 'agent', scoped: false, unscoped: true, changes: 'agent', noun: 'Agent' },
	'actor-session': {
		wire: 'actorSession',
		scoped: false,
		unscoped: true,
		changes: 'actorSession',
		noun: 'Session'
	},
	article: {
		wire: 'article',
		scoped: false,
		unscoped: true,
		changes: 'feedArticles',
		noun: 'Article'
	}
} as const satisfies Record<string, ItemKindInfo>;

type Kinds = typeof ITEM_KINDS;

/** What a `cohesion://` link can point at. */
export type ItemKind = keyof Kinds;

/** What a `cohesion://` link names with its workspace. */
export type WorkspaceItemKind = {
	[K in ItemKind]: Kinds[K]['scoped'] extends true ? K : never;
}[ItemKind];

/** What a `cohesion://` link can name without its workspace. */
export type UnscopedItemKind = {
	[K in ItemKind]: Kinds[K]['unscoped'] extends true ? K : never;
}[ItemKind];

const kinds = Object.keys(ITEM_KINDS) as ItemKind[];

/** Whether `word` is a kind a link names with its workspace. */
export function isWorkspaceItemKind(word: string): word is WorkspaceItemKind {
	return Object.hasOwn(ITEM_KINDS, word) && ITEM_KINDS[word as ItemKind].scoped;
}

/** Whether `word` is a kind a link names without its workspace. */
export function isUnscopedItemKind(word: string): word is UnscopedItemKind {
	return Object.hasOwn(ITEM_KINDS, word) && ITEM_KINDS[word as ItemKind].unscoped;
}

/** The kind of link that names what the daemon calls `wire`. */
export function itemKindOf(wire: LinkKind): ItemKind {
	return kinds.find((kind) => ITEM_KINDS[kind].wire === wire)!;
}

const WORKSPACE_ITEM_LINK = new RegExp(
	`^cohesion://(${kinds.filter(isWorkspaceItemKind).join('|')})/(\\d+)/(\\d+)$`
);
const ITEM_LINK = new RegExp(`^cohesion://(${kinds.filter(isUnscopedItemKind).join('|')})/(\\d+)$`);

/**
 * What a `cohesion://` link names: a task, document, resource, document
 * folder or contact with its workspace (`cohesion://task/3/42`), or an item
 * without one (`cohesion://task/42`, `cohesion://agent/3`), as links to tasks
 * and documents were first written.
 */
export type ItemLink =
	| { kind: WorkspaceItemKind; ws: number; id: number }
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
	const match = ITEM_LINK.exec(href);
	return match ? { kind: match[1] as UnscopedItemKind, ws: null, id: Number(match[2]) } : null;
}

/** `link` as written: `cohesion://task/3/42`. */
export function itemLinkHref(link: ItemLink): string {
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
