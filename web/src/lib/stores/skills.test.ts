import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { IntelligenceAgent } from '$lib/protocol/types';
import { createSkillsStore, matchingSkills, skillsOf } from './skills.svelte';

describe('skills store', () => {
	it('gives an agent its own skills and its team’s, its own winning a name', async () => {
		const mock = createMock();
		const store = createSkillsStore(mock.send);
		await store.load();
		const agents = await mock.send('listIntelligenceAgents');
		const core = agents.find((a) => a.agent_name === 'Core Engineer') as IntelligenceAgent;
		const plus = agents.find((a) => a.agent_name === 'Plus Engineer') as IntelligenceAgent;
		expect(skillsOf(store.list.value, core).map((s) => s.name)).toEqual([
			'Release notes',
			'Standup',
			'Triage'
		]);
		expect(skillsOf(store.list.value, plus).map((s) => s.name)).toEqual(['Release notes']);

		await store.create(
			{ kind: 'agent', agent_id: plus.id },
			{ name: 'release NOTES', description: '', prompt: 'Only iOS.', agent_can_use: true }
		);
		expect(skillsOf(store.list.value, plus).map((s) => [s.name, s.prompt])).toEqual([
			['release NOTES', 'Only iOS.']
		]);
	});

	it('updates and deletes a skill from the reply', async () => {
		const mock = createMock();
		const store = createSkillsStore(mock.send);
		await store.load();
		const [first] = store.list.value;
		const changed = await store.update(first.id, {
			name: 'Changelog',
			description: '',
			prompt: first.prompt,
			agent_can_use: false
		});
		expect(store.list.value.find((s) => s.id === first.id)).toEqual(changed);
		expect(store.list.value[0].name).toBe('Changelog');
		await store.delete(first.id);
		expect(store.list.value.some((s) => s.id === first.id)).toBe(false);
	});

	it('refuses a name its scope already has', async () => {
		const mock = createMock();
		const store = createSkillsStore(mock.send);
		await expect(
			store.create(
				{ kind: 'agent', agent_id: 1 },
				{ name: ' triage ', description: '', prompt: 'x', agent_can_use: true }
			)
		).rejects.toMatchObject({ field: 'name' });
	});

	it('offers the skills whose names start with the query first', async () => {
		const mock = createMock();
		const store = createSkillsStore(mock.send);
		await store.load();
		expect(matchingSkills(store.list.value, 'n').map((s) => s.name)).toEqual([
			'Release notes',
			'Standup'
		]);
		expect(matchingSkills(store.list.value, 'st').map((s) => s.name)).toEqual(['Standup']);
		expect(matchingSkills(store.list.value, '').length).toBe(3);
	});
});
