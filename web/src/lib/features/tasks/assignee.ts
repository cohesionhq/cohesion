import type { Contact, IntelligenceAgent, Profile, TaskAssignee } from '$lib/protocol/types';
import { fullName } from '$lib/stores/contacts.svelte';
import type { Avatar } from '$lib/ui/AgentAvatar.svelte';
import type { Person } from '$lib/ui/Photo.svelte';

/** Who a task is assigned to, as the app shows them. */
export interface ShownAssignee {
	label: string;
	avatar: Avatar | Person;
}

/** One choice of who to assign a task to: the user, an agent or a contact. */
export interface AssigneeChoice extends ShownAssignee {
	/** `user`, `agent-<id>` or `contact-<id>`, unique among the choices; `none` is no one. */
	id: string;
	assignee: TaskAssignee;
}

/** What the user is called where a task is assigned to them. */
const YOU = 'You';
/** What an unassigned task shows, as the panel's other unset rows do. */
export const NO_ONE = 'None';

/** The choice's id for `assignee`. */
export function assigneeId(assignee: TaskAssignee | null): string {
	if (assignee === null) return 'none';
	switch (assignee.kind) {
		case 'user':
			return 'user';
		case 'agent':
			return `agent-${assignee.agent_id}`;
		case 'contact':
			return `contact-${assignee.contact_id}`;
	}
}

function person(profile: Profile | null): Person {
	return { name: profile?.name ?? null, photo: profile?.photo ?? { kind: 'none' } };
}

function agentChoice(agent: IntelligenceAgent): AssigneeChoice {
	const assignee: TaskAssignee = { kind: 'agent', agent_id: agent.id };
	return {
		id: assigneeId(assignee),
		label: agent.agent_name,
		avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji },
		assignee
	};
}

function contactChoice(contact: Contact): AssigneeChoice {
	const assignee: TaskAssignee = { kind: 'contact', contact_id: contact.id };
	const name = fullName(contact);
	return {
		id: assigneeId(assignee),
		label: name,
		avatar: { name, photo: contact.photo },
		assignee
	};
}

/**
 * Who `assignee` is: the user, drawn by their profile's photo or initials,
 * an agent, by its avatar, or a contact, by their photo or initials. Null
 * when no one is, or when the agent or contact is no longer in `agents` or
 * `contacts`: the daemon unassigns a deleted one's tasks, and a list fetched
 * before that shows no one rather than someone stale.
 */
export function shownAssignee(
	assignee: TaskAssignee | null,
	agents: IntelligenceAgent[],
	contacts: Contact[],
	profile: Profile | null
): ShownAssignee | null {
	if (assignee === null) return null;
	switch (assignee.kind) {
		case 'user':
			return { label: YOU, avatar: person(profile) };
		case 'agent': {
			const agent = agents.find((a) => a.id === assignee.agent_id);
			return agent ? agentChoice(agent) : null;
		}
		case 'contact': {
			const contact = contacts.find((c) => c.id === assignee.contact_id);
			return contact ? contactChoice(contact) : null;
		}
	}
}

/**
 * Who a task can be assigned to: the user, then each agent in `agents`'
 * order, then each contact of the task's workspace in `contacts`' order.
 */
export function assigneeChoices(
	agents: IntelligenceAgent[],
	contacts: Contact[],
	profile: Profile | null
): AssigneeChoice[] {
	const user: TaskAssignee = { kind: 'user' };
	return [
		{ id: assigneeId(user), label: YOU, avatar: person(profile), assignee: user },
		...agents.map(agentChoice),
		...contacts.map(contactChoice)
	];
}
