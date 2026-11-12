// Search, what a `cohesion://` link names, embeds and backlinks.

import { parseItemLink, textPieces } from '$lib/format/itemLinks';
import { ProtocolError } from '../../errors';
import type { Embed, SearchResult } from '../../types';
import { searchWords, starts } from '../support';
import type { Handlers, MockCore } from '../core';

export function linksHandlers(core: MockCore) {
	const { state, linksTo, resolveLink } = core;
	return {
		// Like the daemon's index for the queries the UI sends (`"word"*` per
		// word, all required): every word starts a word of the title or
		// contents; title matches first, the shortest title (an exact one)
		// leading.
		searchWorkspace: ({ workspace_id, query }) => {
			const words = searchWords(query);
			if (words.length === 0) return [];
			// How many words a matching title has; Infinity when only the contents matched.
			const hits: { titleWords: number; result: SearchResult }[] = [];
			const consider = (
				entity_kind: SearchResult['entity_kind'],
				entity_id: number,
				title: string,
				contents: string,
				status = 'none'
			) => {
				const [inTitle, inContents] = [starts(title), starts(contents)];
				if (!words.every((w) => inTitle(w) || inContents(w))) return;
				hits.push({
					titleWords: words.some(inTitle) ? searchWords(title).length : Infinity,
					result: { id: 0, entity_kind, entity_id, workspace_id, title, contents, status }
				});
			};
			for (const d of state.documents)
				if (d.workspace_id === workspace_id && !d.deleted)
					consider('document', d.id, d.title, d.contents);
			for (const t of state.tasks)
				if (t.workspace_id === workspace_id && !t.deleted)
					consider('task', t.id, t.title, '', t.status);
			// JSON resources are indexed by title only.
			for (const a of state.resources)
				if (a.workspace_id === workspace_id)
					consider('resource', a.id, a.title, a.content_kind === 'json' ? '' : a.contents);
			// A contact by its full name, with its notes.
			for (const c of state.contacts)
				if (c.workspace_id === workspace_id && !c.deleted)
					consider(
						'contact',
						c.id,
						[c.first_name, c.middle_name, c.last_name].filter(Boolean).join(' '),
						c.notes
					);
			// Folders by title alone.
			for (const f of state.folders)
				if (f.workspace_id === workspace_id && !f.deleted)
					consider('taskFolder', f.id, f.title, '');
			for (const f of state.documentFolders)
				if (f.workspace_id === workspace_id && !f.deleted)
					consider('documentFolder', f.id, f.title, '');
			// Feeds belong to no workspace, so every workspace finds them.
			for (const f of state.feeds) if (!f.deleted) consider('feed', f.id, f.title, '');
			return hits
				.sort((a, b) => (a.titleWords === b.titleWords ? 0 : a.titleWords - b.titleWords))
				.map(({ result }, index) => ({ ...result, id: index + 1 }));
		},
		resolveLink: ({ link }) => structuredClone(resolveLink(link)),
		backlinksOf: ({ link }) => {
			const target = parseItemLink(link.trim());
			if (!target)
				throw new ProtocolError(
					'validationFailed',
					'That isn’t a link to a Cohesion item, such as cohesion://task/3/42.',
					'link'
				);
			const linking: Embed[] = [];
			for (const d of state.documents) {
				const self = target.kind === 'document' && d.id === target.id;
				if (d.deleted || self || !linksTo(d.contents, target)) continue;
				try {
					linking.push(structuredClone(resolveLink(`cohesion://document/${d.id}`)));
				} catch (error) {
					if (!(error instanceof ProtocolError && error.code === 'notFound')) throw error;
				}
			}
			for (const t of state.tasks) {
				const self = target.kind === 'task' && t.id === target.id;
				if (t.deleted || self || !linksTo(t.description, target)) continue;
				try {
					linking.push(structuredClone(resolveLink(`cohesion://task/${t.id}`)));
				} catch (error) {
					if (!(error instanceof ProtocolError && error.code === 'notFound')) throw error;
				}
			}
			return linking.sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase()));
		},
		resolveEmbeds: ({ text }) =>
			textPieces(text).map((piece) => {
				if (piece.kind === 'markdown') return piece;
				const { href: link, label } = piece;
				try {
					return { kind: 'embed', link, label, embed: structuredClone(resolveLink(link)) };
				} catch (error) {
					if (error instanceof ProtocolError && error.code === 'notFound')
						return { kind: 'embed', link, label, embed: null };
					throw error;
				}
			})
	} satisfies Partial<Handlers>;
}
