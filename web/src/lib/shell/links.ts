// Links between items: `cohesion://task/3/42`, `cohesion://document/3/7`
// (workspace, then item), `cohesion://agent/3` and `cohesion://actor-session/143`,
// which agents and the user write as Markdown links, and the first form's
// `cohesion://task/42` and `cohesion://document/7`, which name no workspace.
// Markdown points each at an Open page (`routes.workspaceItem`, `routes.item`),
// which finds the item and goes to it. A click on a link opens the item in a
// panel over the page instead (`linkedItem`), with all its details; one that
// has a page of its own (a document, an agent, an actor session) has a way there.

import { send } from '$lib/protocol/api';
import { ProtocolError } from '$lib/protocol/errors';
import { routes, type ItemKind, type WorkspaceItemKind } from '$lib/shell/routes';
import type { Document, Task } from '$lib/protocol/types';

import { workspaces } from '$lib/stores/workspaces.svelte';
import { mainStores } from '$lib/stores/pageStores.svelte';

const { agents, tasks } = mainStores;

/**
 * Where an item opens, or null once it is deleted or never was. The link
 * names no workspace, so a task or document is looked for in the current
 * workspace first, then the others.
 */
export async function itemRoute(kind: ItemKind, id: number): Promise<string | null> {
	if (kind === 'agent') {
		await agents.loadList();
		return agents.list.value.some((a) => a.id === id) ? routes.agent(id) : null;
	}
	if (kind === 'actor-session')
		return (await found(send('getActorSession', { id }))) !== null ? routes.actorSession(id) : null;
	if (kind === 'task') {
		const task = await findTask(null, id);
		return task === null ? null : routes.task(task.workspace_id, id);
	}
	await workspaces.load();
	for (const ws of searchOrder()) {
		const href = await workspaceItemRoute(kind, ws, id);
		if (href !== null) return href;
	}
	return null;
}

/**
 * Task `id` in workspace `ws`, or null when it has none by `id`. A link naming
 * no workspace (`ws` null) is looked for in the current workspace first, then
 * the others.
 */
export async function findTask(ws: number | null, id: number): Promise<Task | null> {
	if (ws !== null) return tasks.find(ws, id);
	await workspaces.load();
	for (const w of searchOrder()) {
		const task = await tasks.find(w, id);
		if (task !== null) return task;
	}
	return null;
}

/** Where a task or document in workspace `ws` opens, or null when that workspace has none by `id`. */
export async function workspaceItemRoute(
	kind: WorkspaceItemKind,
	ws: number,
	id: number
): Promise<string | null> {
	if (kind === 'task') return (await findTask(ws, id)) !== null ? routes.task(ws, id) : null;
	const document = await findDocument(ws, id);
	return document === null ? null : documentRoute(document);
}

/**
 * Document `id` in workspace `ws`, or null when it has none by `id`; with
 * `ws` null, looked for as `findTask` looks for a task.
 */
export async function findDocument(ws: number | null, id: number): Promise<Document | null> {
	if (ws !== null) return found(send('getDocument', { workspace_id: ws, document_id: id }));
	await workspaces.load();
	for (const w of searchOrder()) {
		const document = await findDocument(w, id);
		if (document !== null) return document;
	}
	return null;
}

/** A document's page, or a journal entry's. */
export function documentRoute(document: Document): string {
	return document.document_type === 'journal'
		? routes.journalEntry(document.workspace_id, document.id)
		: routes.document(document.workspace_id, document.id);
}

/** Every workspace's id, the current one first. */
function searchOrder(): number[] {
	const current = workspaces.currentId;
	const others = workspaces.list.value.map((w) => w.id).filter((id) => id !== current);
	return current === null ? others : [current, ...others];
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
