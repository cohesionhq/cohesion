import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { IntelligenceAgent, SearchResult, Send } from '$lib/protocol/types';
import { MENTION_ROWS, namedBy, searchMentions } from './mentions';

/** A daemon whose workspace search finds `count` tasks, with articles to spare. */
function daemon(count: number, asked: string[]): Send {
	const tasks: SearchResult[] = Array.from({ length: count }, (_, i) => ({
		id: i,
		entity_kind: 'task',
		entity_id: i + 1,
		workspace_id: 1,
		title: `Plan ${i + 1}`,
		contents: ''
	}));
	return (async (type: string, args: { limit?: number }) => {
		asked.push(type);
		if (type === 'searchWorkspace') return tasks;
		if (type === 'searchActorSessions') return [];
		if (type === 'searchFeedCorpus')
			return {
				hits: Array.from({ length: args.limit ?? 50 }, (_, i) => ({
					id: 100 + i,
					title: `Post ${i}`
				})),
				total: 50
			};
		throw new Error(type);
	}) as unknown as Send;
}

describe('@ mentions', () => {
	it('fills only the rows left with articles, after everything else', async () => {
		const asked: string[] = [];
		const found = await searchMentions(daemon(6, asked), 1, 'pla', []);
		expect(found).toHaveLength(MENTION_ROWS);
		expect(found.slice(0, 6).every((m) => m.href.startsWith('cohesion://task/1/'))).toBe(true);
		expect(found.slice(6).map((m) => m.href)).toEqual([
			'cohesion://article/100',
			'cohesion://article/101'
		]);
		expect(found[0]).toMatchObject({ label: 'Plan 1', detail: 'Task #1' });
	});

	it('asks for no articles when the rows are full', async () => {
		const asked: string[] = [];
		await searchMentions(daemon(MENTION_ROWS + 2, asked), 1, 'pla', []);
		expect(asked).not.toContain('searchFeedCorpus');
	});

	it('puts agents named by the text first', async () => {
		const scout = { id: 3, agent_name: 'Scout' } as IntelligenceAgent;
		const found = await searchMentions(daemon(1, []), 1, 'sco', [scout]);
		expect(found[0]).toEqual({ label: 'Scout', detail: 'Agent', href: 'cohesion://agent/3' });
	});

	it('finds the mock daemon’s items and links them', async () => {
		const mock = createMock();
		const document = mock.state.documents.find((d) => !d.deleted && d.workspace_id === 1)!;
		const word = document.title.split(' ')[0];
		const found = await searchMentions(mock.send, 1, word, []);
		expect(found.map((m) => m.href)).toContain(`cohesion://document/1/${document.id}`);
	});

	it('matches a name by the starts of its words', () => {
		expect(namedBy('Research Scout', 'sco res')).toBe(true);
		expect(namedBy('Research Scout', 'out')).toBe(false);
		expect(namedBy('Scout', '')).toBe(false);
	});
});
