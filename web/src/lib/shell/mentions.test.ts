import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { IntelligenceAgent, SearchResult, Send } from '$lib/protocol/types';
import { MENTION_ROWS, namedBy, searchMentions } from './mentions';

/** A daemon whose workspace search finds `count` tasks. */
function daemon(count: number, asked: string[]): Send {
	const tasks: SearchResult[] = Array.from({ length: count }, (_, i) => ({
		id: i,
		entity_kind: 'task',
		entity_id: i + 1,
		workspace_id: 1,
		title: `Plan ${i + 1}`,
		contents: '',
		status: 'incomplete'
	}));
	return (async (type: string) => {
		asked.push(type);
		if (type === 'searchWorkspace') return tasks;
		if (type === 'searchActorSessions') return [];
		throw new Error(type);
	}) as unknown as Send;
}

describe('@ mentions', () => {
	it('offers the items and never articles', async () => {
		const asked: string[] = [];
		const found = await searchMentions(daemon(6, asked), 1, 'pla', []);
		expect(found).toHaveLength(6);
		expect(found.every((m) => m.href.startsWith('cohesion://task/1/'))).toBe(true);
		expect(found[0]).toMatchObject({ label: 'Plan 1', detail: 'Task #1' });
		expect(asked).not.toContain('searchFeedCorpus');
	});

	it('stops at the most rows', async () => {
		const found = await searchMentions(daemon(MENTION_ROWS + 2, []), 1, 'pla', []);
		expect(found).toHaveLength(MENTION_ROWS);
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

	it('finds a contact by name and links it in its workspace', async () => {
		const mock = createMock();
		const found = await searchMentions(mock.send, 1, 'okaf', []);
		expect(found).toContainEqual({
			label: 'Mira Okafor',
			detail: 'Contact',
			href: 'cohesion://contact/1/1'
		});
		const embed = await mock.send('resolveLink', { link: 'cohesion://contact/1/1' });
		expect(embed).toMatchObject({ kind: 'contact', title: 'Mira Okafor', workspace_id: 1 });
	});

	it('matches a name by the starts of its words', () => {
		expect(namedBy('Research Scout', 'sco res')).toBe(true);
		expect(namedBy('Research Scout', 'out')).toBe(false);
		expect(namedBy('Scout', '')).toBe(false);
	});
});
