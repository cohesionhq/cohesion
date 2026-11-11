// Skills: prompts the user saves for one agent or for every agent on a team.
// The user runs any of an agent's from its composer; the agent sees those it
// may use and runs them itself.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	AgentSkill,
	AgentSkillScope,
	IntelligenceAgent,
	Invalidation,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

/** What the skill dialog edits: everything but whose it is. */
export interface SkillFields {
	name: string;
	description: string;
	prompt: string;
	agent_can_use: boolean;
}

/** Whether `skill` is `scope`'s. */
export function inScope(skill: AgentSkill, scope: AgentSkillScope): boolean {
	if (skill.scope.kind === 'agent' && scope.kind === 'agent')
		return skill.scope.agent_id === scope.agent_id;
	if (skill.scope.kind === 'team' && scope.kind === 'team')
		return skill.scope.team_id === scope.team_id;
	return false;
}

/**
 * The skills `agent` has, by name: its own, and its team's whose names none
 * of its own take, since its own wins. The daemon picks the same ones.
 */
export function skillsOf(skills: AgentSkill[], agent: IntelligenceAgent): AgentSkill[] {
	const own = skills.filter((s) => inScope(s, { kind: 'agent', agent_id: agent.id }));
	const taken = own.map((s) => s.name.toLowerCase());
	const team =
		agent.team_id === null
			? []
			: skills.filter(
					(s) =>
						inScope(s, { kind: 'team', team_id: agent.team_id! }) &&
						!taken.includes(s.name.toLowerCase())
				);
	return [...own, ...team].sort(byName);
}

/** The skills whose names hold `query`, ignoring case: those starting with it first. */
export function matchingSkills(skills: AgentSkill[], query: string): AgentSkill[] {
	const q = query.trim().toLowerCase();
	if (q === '') return skills;
	const starts = skills.filter((s) => s.name.toLowerCase().startsWith(q));
	const within = skills.filter(
		(s) => !s.name.toLowerCase().startsWith(q) && s.name.toLowerCase().includes(q)
	);
	return [...starts, ...within];
}

function byName(a: AgentSkill, b: AgentSkill): number {
	return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }) || a.id - b.id;
}

const skillsChanged = (key: Invalidation) => key.kind === 'agentSkills';

export class SkillsStore {
	/** Every skill, each agent's and each team's, by name. */
	readonly list: Resource<AgentSkill[]>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: () => send('listAgentSkills'),
			initial: [],
			matches: skillsChanged
		});
	}

	load(): Promise<void> {
		return this.list.load();
	}

	async create(scope: AgentSkillScope, fields: SkillFields): Promise<AgentSkill> {
		const skill = await this.#send('createAgentSkill', { scope, ...fields });
		this.list.update((list) => [...list, skill].sort(byName));
		return skill;
	}

	async update(skill_id: number, fields: SkillFields): Promise<AgentSkill> {
		const skill = await this.#send('updateAgentSkill', { skill_id, ...fields });
		this.list.update((list) => list.map((s) => (s.id === skill.id ? skill : s)).sort(byName));
		return skill;
	}

	async delete(skill_id: number): Promise<AgentSkill> {
		const skill = await this.#send('deleteAgentSkill', { skill_id });
		this.list.update((list) => list.filter((s) => s.id !== skill.id));
		return skill;
	}
}

export function createSkillsStore(send: Send = defaultSend): SkillsStore {
	return new SkillsStore(send);
}

export const skills = createSkillsStore();
