// What `@` or `[[` finds in the editor: items to link by title, in the order they're
// offered. Articles are not offered: copy an article's link and paste it.

import { itemLinkHref } from '$lib/format/itemLinks';
import { prefixQuery } from '$lib/format/search';
import type { IntelligenceAgent, Send } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import type { Mention } from '$lib/ui/Editor.svelte';

/** The most rows `@` and `[[` offer. */
export const MENTION_ROWS = 8;

/**
 * The items in workspace `ws` (documents and journal entries, tasks,
 * resources, contacts, document folders), the agents among `agents` and the actor sessions matching
 * `text`. A search that fails offers nothing of its kind.
 */
export async function searchMentions(
	send: Send,
	ws: number,
	text: string,
	agents: IntelligenceAgent[]
): Promise<Mention[]> {
	const query = prefixQuery(text);
	if (query === null) return [];
	const word = vocabulary.embedKind.bind(vocabulary);
	const [items, sessions] = await Promise.all([
		send('searchWorkspace', { workspace_id: ws, query }).catch(() => []),
		send('searchActorSessions', { query }).catch(() => [])
	]);
	return [
		...agents
			.filter((agent) => namedBy(agent.agent_name, text))
			.map((agent) => ({
				label: agent.agent_name,
				detail: word('agent'),
				href: itemLinkHref({ kind: 'agent', ws: null, id: agent.id })
			})),
		// A task folder has no link of its own, so it isn't offered.
		...items.flatMap((hit) => {
			if (hit.entity_kind === 'taskFolder') return [];
			const kind = hit.entity_kind === 'documentFolder' ? 'document-folder' : hit.entity_kind;
			return [
				{
					label: hit.title ?? '',
					detail:
						hit.entity_kind === 'task'
							? `${word('task')} #${hit.entity_id}`
							: word(hit.entity_kind),
					href: itemLinkHref({ kind, ws: hit.workspace_id, id: hit.entity_id })
				}
			];
		}),
		...sessions.map((hit) => ({
			label: hit.title,
			detail: word('actorSession'),
			href: itemLinkHref({ kind: 'actor-session', ws: null, id: hit.id })
		}))
	]
		.filter((mention) => mention.label.trim() !== '')
		.slice(0, MENTION_ROWS);
}

/** Whether every word of `text` starts a word of `name`, as the search matches titles. */
export function namedBy(name: string, text: string): boolean {
	const words = (name.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
	const typed = (text.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
	return typed.length > 0 && typed.every((t) => words.some((w) => w.startsWith(t)));
}
