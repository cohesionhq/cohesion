// Settings → Memories: what agents keep with the remember tool, the global
// memories every agent sees, each team's and each agent's own.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	IntelligenceAgent,
	IntelligenceAgentTeam,
	Invalidation,
	Memory,
	Send
} from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

/** One agent and its own memories, oldest first. */
export interface AgentMemories {
	agent: IntelligenceAgent;
	memories: Memory[];
}

/** One team and the memories its agents share, oldest first. */
export interface TeamMemories {
	team: IntelligenceAgentTeam;
	memories: Memory[];
}

export interface MemoryList {
	/** The memories every agent sees, oldest first. */
	global: Memory[];
	/** Each team's memories, by title, empty ones included. */
	teams: TeamMemories[];
	/** Each agent's own memories, in the agents list's order, empty ones included. */
	agents: AgentMemories[];
}

/** A memory remembered or forgotten, or an agent or team changed or deleted. */
const memoriesChanged = (key: Invalidation) =>
	key.kind === 'memories' || key.kind === 'agent' || key.kind === 'agentTeams';

export class MemoriesStore {
	readonly list: Resource<MemoryList>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			// An agent's list holds its team's and the global memories too; each shows once, under
			// its own heading. A team's are read through its agents, so a team with none shows none.
			fetch: async () => {
				const [global, agents, teams] = await Promise.all([
					send('listMemories', { agent_id: null }),
					send('listIntelligenceAgents'),
					send('listAgentTeams')
				]);
				const seen = (
					await Promise.all(agents.map((agent) => send('listMemories', { agent_id: agent.id })))
				).flat();
				const teamMemories = (team_id: number) =>
					seen.filter(
						(m, i) =>
							m.scope.kind === 'team' &&
							m.scope.team_id === team_id &&
							seen.findIndex((other) => other.id === m.id) === i
					);
				return {
					global,
					teams: teams.map((team) => ({ team, memories: teamMemories(team.id) })),
					agents: agents.map((agent) => ({
						agent,
						memories: seen.filter((m) => m.scope.kind === 'agent' && m.scope.agent_id === agent.id)
					}))
				};
			},
			initial: { global: [], teams: [], agents: [] },
			matches: memoriesChanged
		});
	}

	load(): Promise<void> {
		return this.list.load();
	}

	release(): void {
		this.list.release();
	}

	/** The memories `agent_id` sees now: its own, its team's and the global ones, oldest first. */
	seenBy(agent_id: number): Promise<Memory[]> {
		return this.#send('listMemories', { agent_id });
	}

	/** Forgets a memory; no agent sees it again. */
	async forget(memory_id: number): Promise<Memory> {
		const forgotten = await this.#send('forgetMemory', { memory_id });
		const without = (memories: Memory[]) => memories.filter((m) => m.id !== forgotten.id);
		this.list.update(({ global, teams, agents }) => ({
			global: without(global),
			teams: teams.map((t) => ({ ...t, memories: without(t.memories) })),
			agents: agents.map((a) => ({ ...a, memories: without(a.memories) }))
		}));
		return forgotten;
	}
}

export function createMemoriesStore(send: Send = defaultSend): MemoriesStore {
	return new MemoriesStore(send);
}

export const memories = createMemoriesStore();
