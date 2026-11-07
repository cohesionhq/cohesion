import { describe, expect, it } from 'vitest';
import type { Contact, IntelligenceAgent, Profile } from '$lib/protocol/types';
import { assigneeChoices, assigneeId, shownAssignee } from './assignee';

const agent = (id: number, name: string) =>
	({ id, agent_name: name, agent_color: 'blue', agent_emoji: null }) as IntelligenceAgent;
const agents = [agent(1, 'Scout'), agent(2, 'Pilot')];
const contacts = [
	{ id: 7, first_name: 'Mira', middle_name: null, last_name: 'Okafor', photo: { kind: 'none' } }
] as Contact[];
const profile: Profile = { id: 1, name: 'Ada Lovelace', bio: null, photo: { kind: 'none' } };

describe('assignees', () => {
	it('shows the user by their profile and an agent by its avatar', () => {
		expect(shownAssignee({ kind: 'user' }, agents, contacts, profile)).toEqual({
			label: 'You',
			avatar: { name: 'Ada Lovelace', photo: { kind: 'none' } }
		});
		expect(shownAssignee({ kind: 'user' }, agents, contacts, null)?.avatar).toEqual({
			name: null,
			photo: { kind: 'none' }
		});
		expect(shownAssignee({ kind: 'agent', agent_id: 2 }, agents, contacts, profile)).toMatchObject({
			label: 'Pilot',
			avatar: { name: 'Pilot', color: 'blue', emoji: null }
		});
		expect(
			shownAssignee({ kind: 'contact', contact_id: 7 }, agents, contacts, profile)
		).toMatchObject({
			label: 'Mira Okafor',
			avatar: { name: 'Mira Okafor', photo: { kind: 'none' } }
		});
	});

	it('shows no one for an unassigned task or an agent that is gone', () => {
		expect(shownAssignee(null, agents, contacts, profile)).toBeNull();
		expect(shownAssignee({ kind: 'agent', agent_id: 9 }, agents, contacts, profile)).toBeNull();
		expect(shownAssignee({ kind: 'contact', contact_id: 9 }, agents, contacts, profile)).toBeNull();
	});

	it('offers the user, then each agent, then each contact, with ids unique among them', () => {
		const choices = assigneeChoices(agents, contacts, profile);
		expect(choices.map((c) => [c.id, c.label])).toEqual([
			['user', 'You'],
			['agent-1', 'Scout'],
			['agent-2', 'Pilot'],
			['contact-7', 'Mira Okafor']
		]);
		for (const choice of choices) expect(assigneeId(choice.assignee)).toBe(choice.id);
		expect(assigneeId(null)).toBe('none');
	});
});
