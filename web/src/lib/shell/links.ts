// Links between items: `cohesion://task/3/42`, `cohesion://document/3/7`,
// `cohesion://resource/3/9` and `cohesion://database/3/2` (workspace, then
// item), `cohesion://database-row/3/2/14` (workspace, database, then row),
// `cohesion://agent/3`,
// `cohesion://actor-session/143` and `cohesion://article/88`, which agents and
// the user write as Markdown links, and the first form's `cohesion://task/42`
// and `cohesion://document/7`, which name no workspace. The daemon finds what a
// link names (`resolveLink`), the same for every client. Markdown points each
// at an Open page (`routes.workspaceItem`, `routes.item`), which goes to the
// item. A click on a link opens the item in a panel over the page instead
// (`linkedItem`), with all its details; one that has a page of its own has a
// way there. A link alone on its line shows the item as a card (`embeds`).

import { send } from '$lib/protocol/api';
import { ProtocolError } from '$lib/protocol/errors';
import type { Document, Embed } from '$lib/protocol/types';
import { itemLinkHref, routes, type ItemLink } from '$lib/shell/routes';

import { workspaces } from '$lib/stores/workspaces.svelte';

/** What `link` names, or null once it is deleted or never was. */
export function resolve(link: ItemLink): Promise<Embed | null> {
	return found(send('resolveLink', { link: itemLinkHref(link) }));
}

/** Where the item `link` names opens, or null once it is deleted or never was. */
export async function itemRoute(link: ItemLink): Promise<string | null> {
	const embed = await resolve(link);
	return embed && embedRoute(embed);
}

/**
 * Where the item `embed` shows opens: its page, the Tasks page at it, or a
 * database row's table with the row open over it. An article belongs to no workspace, so it opens
 * in the current one; with no workspace at all it has nowhere to open (null).
 */
export async function embedRoute(embed: Embed): Promise<string | null> {
	const { item } = embed;
	switch (item.kind) {
		case 'task':
			return routes.task(item.task.workspace_id, item.task.id);
		case 'document':
			return documentRoute(item.document);
		case 'resource':
			return routes.resource(item.resource.workspace_id, item.resource.kind_id, item.resource.id);
		case 'agent':
			return routes.agent(item.agent.id);
		case 'actorSession':
			return routes.actorSession(item.session.id);
		case 'article':
			return articleRoute(item.article.id);
		case 'database':
			return routes.database(item.database.workspace_id, item.database.id);
		case 'databaseRow':
			return embed.workspace_id === null
				? null
				: routes.databaseRow(embed.workspace_id, item.row.database_id, item.row.id);
	}
}

/** Article `id`'s page, in the current workspace (else the first), or null with none. */
export async function articleRoute(id: number): Promise<string | null> {
	await workspaces.load();
	const ws = workspaces.currentId ?? workspaces.list.value[0]?.id;
	return ws === undefined ? null : routes.article(ws, id);
}

/** A document's page, or a journal entry's. */
export function documentRoute(document: Document): string {
	return document.document_type === 'journal'
		? routes.journalEntry(document.workspace_id, document.id)
		: routes.document(document.workspace_id, document.id);
}

/** What `request` returns, or null when it is not found. */
export async function found<T>(request: Promise<T>): Promise<T | null> {
	try {
		return await request;
	} catch (error) {
		if (error instanceof ProtocolError && error.code === 'notFound') return null;
		throw error;
	}
}

/**
 * The path a click on `target` follows when it lands on a link to a page of
 * this app that opens in place (no `target`, no `download`), or null.
 */
export function internalHref(target: EventTarget | null): string | null {
	const link = target instanceof Element ? target.closest('a[href]') : null;
	if (!(link instanceof HTMLAnchorElement) || link.target || link.hasAttribute('download'))
		return null;
	const url = new URL(link.href);
	return url.origin === location.origin ? url.pathname + url.search + url.hash : null;
}
