import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	ITEM_KINDS,
	isUnscopedItemKind,
	isWorkspaceItemKind,
	itemKindOf,
	itemLinkHref,
	parseItemLink,
	textPieces,
	type ItemKind,
	type TextPiece
} from './itemLinks';

const markdown = (text: string): TextPiece => ({ kind: 'markdown', text });
const task = (label: string, ws: number, id: number): TextPiece => ({
	kind: 'embed',
	link: { kind: 'task', ws, id },
	href: `cohesion://task/${ws}/${id}`,
	label
});

describe('item links', () => {
	it('reads each kind in the forms its entry allows, and no other', () => {
		for (const kind of Object.keys(ITEM_KINDS) as ItemKind[]) {
			const { scoped, unscoped, wire } = ITEM_KINDS[kind];
			expect(scoped || unscoped).toBe(true);
			expect(isWorkspaceItemKind(kind)).toBe(scoped);
			expect(isUnscopedItemKind(kind)).toBe(unscoped);
			expect(parseItemLink(`cohesion://${kind}/3/42`)).toEqual(
				scoped ? { kind, ws: 3, id: 42 } : null
			);
			expect(parseItemLink(`cohesion://${kind}/42`)).toEqual(
				unscoped ? { kind, ws: null, id: 42 } : null
			);
			expect(itemKindOf(wire)).toBe(kind);
		}
		expect(isWorkspaceItemKind('toString')).toBe(false);
		expect(isUnscopedItemKind('journal')).toBe(false);
	});

	it('reads and writes back every kind', () => {
		for (const href of [
			'cohesion://task/3/42',
			'cohesion://task/42',
			'cohesion://document/3/7',
			'cohesion://resource/3/9',
			'cohesion://document-folder/3/2',
			'cohesion://contact/3/12',
			'cohesion://agent/3',
			'cohesion://actor-session/143',
			'cohesion://article/88'
		])
			expect(itemLinkHref(parseItemLink(href)!)).toBe(href);
	});

	it('reads a document folder with its workspace', () => {
		expect(parseItemLink('cohesion://document-folder/3/2')).toEqual({
			kind: 'document-folder',
			ws: 3,
			id: 2
		});
	});

	it('reads nothing else', () => {
		for (const href of [
			'cohesion://resource/9',
			'cohesion://article/1/2',
			'cohesion://contact/3',
			'cohesion://document-folder/2',
			'cohesion://database/3/2',
			'cohesion://database-row/3/2/14',
			'https://example.com'
		])
			expect(parseItemLink(href)).toBeNull();
	});
});

// The daemon's `text_pieces` (cohesion-core/src/model/text_section/tests.rs)
// splits these same texts the same way.
describe('text pieces', () => {
	it('makes a link alone on its line an embed between Markdown', () => {
		expect(
			textPieces('# Plan\n\nFirst:\n\n[Fix login](cohesion://task/3/42)\n\nThen the rest.')
		).toEqual([markdown('# Plan\n\nFirst:'), task('Fix login', 3, 42), markdown('Then the rest.')]);
	});

	it('leaves a link in a sentence or a list as Markdown', () => {
		const text =
			'See [Fix login](cohesion://task/3/42) today.\n- [Fix login](cohesion://task/3/42)';
		expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('leaves a link in code as Markdown', () => {
		for (const text of [
			'```\n[Fix login](cohesion://task/3/42)\n```',
			'    [Fix login](cohesion://task/3/42)',
			'\t[Fix login](cohesion://task/3/42)'
		])
			expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('leaves a link to anything but an item as Markdown', () => {
		for (const text of [
			'[Example](https://example.com)',
			'[Contact](cohesion://contact/3)',
			'![Chart](cohesion://task/3/42)'
		])
			expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('leaves a sentence whose last link ends the line as Markdown', () => {
		const text = '[Spec](https://example.com) blocks [Fix login](cohesion://task/5/42)';
		expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('leaves two links on one line as Markdown', () => {
		for (const text of [
			'[A](cohesion://task/3/1) [B](cohesion://task/3/2)',
			'[A](cohesion://task/3/1)[B](cohesion://task/3/2)'
		])
			expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('keeps an escaped bracket in the label', () => {
		expect(textPieces('[Fix \\] login \\[2\\]](cohesion://task/3/42)')).toEqual([
			task('Fix ] login [2]', 3, 42)
		]);
		// The same label unescaped ends at its first `]`, and what follows is text.
		const text = '[Fix ] login](cohesion://task/3/42)';
		expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('leaves a bare link as Markdown', () => {
		for (const text of ['cohesion://task/3/42', '<cohesion://task/3/42>', '(cohesion://task/3/42)'])
			expect(textPieces(text)).toEqual([markdown(text)]);
	});

	it('needs no Markdown between embeds side by side', () => {
		expect(textPieces('  [A](cohesion://task/3/1)  \n[B](cohesion://task/3/2)\n\n')).toEqual([
			task('A', 3, 1),
			task('B', 3, 2)
		]);
	});

	it('splits nothing into nothing', () => {
		expect(textPieces('')).toEqual([]);
		expect(textPieces('\n \n')).toEqual([]);
	});
});

// The cases the daemon's tests run too (`the_shared_link_cases_hold`,
// `the_shared_text_cases_hold` in cohesion-core): the web UI must read a
// link, and find a text's embeds, as the daemon does. When a case fails only
// here, this side is what's wrong.
describe('the link cases shared with the daemon', () => {
	const shared: {
		links: {
			href: string;
			names: { kind: string; workspace_id: number | null; id: number } | null;
		}[];
		texts: {
			text: string;
			pieces: ({ markdown: string } | { embed: { label: string; href: string } })[];
		}[];
	} = JSON.parse(
		readFileSync(
			new URL('../../../../cohesion-core/src/model/item_link/link_cases.json', import.meta.url),
			'utf8'
		)
	);

	it('reads each link as the daemon does', () => {
		for (const { href, names } of shared.links) {
			const link = parseItemLink(href);
			const read = link && {
				kind: ITEM_KINDS[link.kind].wire,
				workspace_id: link.ws,
				id: link.id
			};
			expect(read, href).toEqual(names);
		}
	});

	it('finds each text’s embeds as the daemon does', () => {
		for (const { text, pieces } of shared.texts) {
			const found = textPieces(text).map((piece) =>
				piece.kind === 'markdown'
					? { markdown: piece.text }
					: { embed: { label: piece.label, href: piece.href } }
			);
			expect(found, JSON.stringify(text)).toEqual(pieces);
		}
	});
});
