// Route builders, so links and the palette agree on paths. Each goes
// through `resolve()` so a configured base path is honoured.

import { resolve } from '$app/paths';
import type { ItemLink, UnscopedItemKind, WorkspaceItemKind } from '$lib/format/itemLinks';

// Links to items are read and written in `$lib/format/itemLinks.ts`, which
// the mock and the stores share.
export {
	ITEM_KINDS,
	itemKindOf,
	itemLinkHref,
	parseItemLink,
	type ItemKind,
	type ItemLink,
	type UnscopedItemKind,
	type WorkspaceItemKind
} from '$lib/format/itemLinks';

/** Where an item a link names goes in the app: its Open page. */
export function itemLinkRoute(link: ItemLink): string {
	return link.ws === null
		? routes.item(link.kind, link.id)
		: routes.workspaceItem(link.kind, link.ws, link.id);
}

export const routes = {
	/** Where a workspace opens: its Dashboard. */
	workspace: (ws: number) => routes.dashboard(ws),
	dashboard: (ws: number) => resolve('/w/[ws]/dashboard', { ws: String(ws) }),
	today: (ws: number) => resolve('/w/[ws]/today', { ws: String(ws) }),
	journal: (ws: number) => resolve('/w/[ws]/journal', { ws: String(ws) }),
	journalEntry: (ws: number, id: number) =>
		resolve('/w/[ws]/journal/[id]', { ws: String(ws), id: String(id) }),
	documents: (ws: number) => resolve('/w/[ws]/documents', { ws: String(ws) }),
	document: (ws: number, id: number) =>
		resolve('/w/[ws]/documents/[id]', { ws: String(ws), id: String(id) }),
	tasks: (ws: number) => resolve('/w/[ws]/tasks', { ws: String(ws) }),
	/** The Documents page at a folder, which shows as a table when it is one. */
	documentFolder: (ws: number, id: number) => `${routes.documents(ws)}?folder=${id}`,
	contacts: (ws: number) => resolve('/w/[ws]/contacts', { ws: String(ws) }),
	/** The Contacts page with contact `id`'s panel open. */
	contact: (ws: number, id: number) =>
		`${resolve('/w/[ws]/contacts', { ws: String(ws) })}?contact=${id}`,
	rewind: (ws: number) => resolve('/w/[ws]/rewind', { ws: String(ws) }),
	/** The Articles page, at one view (`viewParam` spells it) or the one last shown. */
	articles: (ws: number, view?: string) =>
		`${resolve('/w/[ws]/articles', { ws: String(ws) })}${view ? `?view=${view}` : ''}`,
	/** The Articles page with its add-feed dialog open. */
	addFeed: (ws: number) => `${resolve('/w/[ws]/articles', { ws: String(ws) })}?add=feed`,
	/** Research: searching the feed corpus. */
	research: (ws: number) => resolve('/w/[ws]/articles/research', { ws: String(ws) }),
	article: (ws: number, id: number) =>
		resolve('/w/[ws]/articles/[id]', { ws: String(ws), id: String(id) }),
	/** The Tasks page, opened at the view holding task `id` with it focused. */
	task: (ws: number, id: number) => `${resolve('/w/[ws]/tasks', { ws: String(ws) })}?task=${id}`,
	/** Finds a linked item and goes to it, as a `cohesion://` link does. */
	item: (kind: UnscopedItemKind, id: number) =>
		resolve('/open/[kind=item]/[id]', { kind, id: String(id) }),
	/** Finds a linked task, document, resource, folder or contact in its workspace and goes to it. */
	workspaceItem: (kind: WorkspaceItemKind, ws: number, id: number) =>
		resolve('/open/[kind=workspaceitem]/[ws]/[id]', { kind, ws: String(ws), id: String(id) }),
	/** A model thread's transcript, read-only. */
	thread: (id: number) => resolve('/threads/[thread]', { thread: String(id) }),
	agents: () => resolve('/agents'),
	/** The Agents page with its new-agent dialog open. */
	newAgent: () => `${resolve('/agents')}?new`,
	agent: (id: number) => resolve('/agents/[agent]', { agent: String(id) }),
	/** One kind's resources in a workspace, which its sidebar link opens. */
	resources: (ws: number, kind: number) =>
		resolve('/w/[ws]/resources/[kind]', { ws: String(ws), kind: String(kind) }),
	resource: (ws: number, kind: number, id: number) =>
		resolve('/w/[ws]/resources/[kind]/[id]', {
			ws: String(ws),
			kind: String(kind),
			id: String(id)
		}),
	/** Every actor the clients offer, with its sessions. */
	actors: () => resolve('/actors'),
	/** One actor session's transcript. */
	actorSession: (id: number) => resolve('/actors/[session]', { session: String(id) }),
	/** The long-running work the user follows, running first. */
	connectorAccounts: () => resolve('/connectors/accounts'),
	/** Extensions opens on its Installed tab. */
	extensions: () => resolve('/extensions'),
	extensionsInstalled: () => resolve('/extensions/installed'),
	extensionsBrowse: () => resolve('/extensions/browse'),
	extension: (id: number) => resolve('/extensions/installed/[id]', { id: String(id) }),
	settings: (section?: string) =>
		section ? resolve('/settings/[section]', { section }) : resolve('/settings')
};

/**
 * The pages the sidebar and ⌘K open, whose code the app loads once it's
 * idle so a first visit doesn't wait on it. An agent's page is the same code
 * for every agent, so any id names it.
 */
export function mainPages(ws: number): string[] {
	return [
		routes.dashboard(ws),
		routes.today(ws),
		routes.journal(ws),
		routes.documents(ws),
		routes.tasks(ws),
		routes.articles(ws),
		routes.contacts(ws),
		routes.rewind(ws),
		routes.agents(),
		routes.agent(0),
		routes.actors(),
		routes.connectorAccounts(),
		routes.extensionsInstalled(),
		routes.settings()
	];
}
