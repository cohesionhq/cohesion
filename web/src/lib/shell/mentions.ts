// What `@` or `[[` finds in the editor: items to link by title, in the order they're
// offered. Articles, shared by every workspace and far more numerous, rank
// last: they fill only the rows nothing else took.

import { prefixQuery } from '$lib/format/search';
import type { IntelligenceAgent, Send } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import type { Mention } from '$lib/ui/Editor.svelte';

/** The most rows `@` and `[[` offer. */
export const MENTION_ROWS = 8;

/**
 * The items in workspace `ws` (documents and journal entries, tasks,
 * resources), the agents among `agents` and the actor sessions matching
 * `text`, then articles to fill the rows left. A search that fails offers
 * nothing of its kind.
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
	const found: Mention[] = [
		...agents
			.filter((agent) => namedBy(agent.agent_name, text))
			.map((agent) => ({
				label: agent.agent_name,
				detail: word('agent'),
				href: `cohesion://agent/${agent.id}`
			})),
		...items.map((hit) => ({
			label: hit.title ?? '',
			detail:
				hit.entity_kind === 'task' ? `${word('task')} #${hit.entity_id}` : word(hit.entity_kind),
			href: `cohesion://${hit.entity_kind}/${hit.workspace_id}/${hit.entity_id}`
		})),
		...sessions.map((hit) => ({
			label: hit.title,
			detail: word('actorSession'),
			href: `cohesion://actor-session/${hit.id}`
		}))
	]
		.filter((mention) => mention.label.trim() !== '')
		.slice(0, MENTION_ROWS);
	const left = MENTION_ROWS - found.length;
	if (left === 0) return found;
	const articles = await send('searchFeedCorpus', { query, limit: left }).catch(() => ({
		hits: []
	}));
	return [
		...found,
		...articles.hits.slice(0, left).map((hit) => ({
			label: hit.title,
			detail: word('article'),
			href: `cohesion://article/${hit.id}`
		}))
	];
}

/** Whether every word of `text` starts a word of `name`, as the search matches titles. */
export function namedBy(name: string, text: string): boolean {
	const words = (name.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
	const typed = (text.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
	return typed.length > 0 && typed.every((t) => words.some((w) => w.startsWith(t)));
}
