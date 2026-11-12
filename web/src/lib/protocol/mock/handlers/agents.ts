// Agents: teams, messages, skills, cues, memories, activities and runs.

import { ProtocolError } from '../../errors';
import type {
	AgentMessageContent,
	AgentSkill,
	IntelligenceAgent,
	IntelligenceAgentTeam,
	Memory
} from '../../types';
import { agentMessage, costEvent, reactionExcerpt } from '../seed';
import { nowSeconds } from '../support';
import type { Handlers, MockCore } from '../core';

export function agentsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		notFound,
		document,
		task,
		workspace,
		emojiValue,
		agent,
		agentRow,
		agentTeam,
		skillFields,
		byName,
		teamTitle,
		sortedTeams,
		sortedAgents,
		agentMessageById
	} = core;
	return {
		listIntelligenceAgents: () => sortedAgents().map(agentRow),
		reorderIntelligenceAgents: ({ agent_ids }) => {
			agent_ids.forEach((id, i) => (agent(id).sort_order = i));
			return sortedAgents().map(agentRow);
		},
		setAgentPinned: ({ agent_id, pinned }) => {
			const a = agent(agent_id);
			if (pinned && !a.pinned)
				a.sort_order =
					Math.max(0, ...state.agents.filter((o) => o.pinned).map((o) => o.sort_order)) + 1;
			a.pinned = pinned;
			return agentRow(a);
		},
		getIntelligenceAgent: ({ agent_id }) => agentRow(agent(agent_id)),
		setAgentTeam: ({ agent_id, team_id }) => {
			const a = agent(agent_id);
			if (team_id != null) agentTeam(team_id);
			a.team_id = team_id ?? null;
			return agentRow(a);
		},
		setAgentModel: ({ agent_id, endpoint_model_id }) => {
			const a = agent(agent_id);
			if (
				endpoint_model_id != null &&
				!state.endpointModels.some((m) => m.id === endpoint_model_id)
			)
				notFound('endpoint model');
			a.endpoint_model_id = endpoint_model_id ?? null;
			return agentRow(a);
		},
		listAgentTeams: () => sortedTeams(),
		createAgentTeam: ({ title }) => {
			const created: IntelligenceAgentTeam = {
				id: nextId(state.agentTeams),
				title: teamTitle(title)
			};
			state.agentTeams.push(created);
			return { ...created };
		},
		renameAgentTeam: ({ team_id, title }) => {
			const t = agentTeam(team_id);
			t.title = teamTitle(title, team_id);
			return { ...t };
		},
		deleteAgentTeam: ({ team_id }) => {
			const t = agentTeam(team_id);
			state.agentTeams = state.agentTeams.filter((x) => x.id !== team_id);
			for (const a of state.agents) if (a.team_id === team_id) a.team_id = null;
			state.memories = state.memories.filter(
				(m) => m.scope.kind !== 'team' || m.scope.team_id !== team_id
			);
			state.agentSkills = state.agentSkills.filter(
				(s) => s.scope.kind !== 'team' || s.scope.team_id !== team_id
			);
			return { ...t };
		},
		createIntelligenceAgent: ({
			name,
			color,
			emoji,
			personality,
			verbosity,
			role,
			directive,
			team_id
		}) => {
			const created: IntelligenceAgent = {
				id: nextId(state.agents),
				created_at: nowSeconds(),
				// The daemon picks an unused first name; the mock numbers them.
				agent_name: name?.trim() || `Agent ${state.agents.length + 1}`,
				agent_color: color,
				agent_emoji: emojiValue(emoji),
				agent_personality: personality ?? 'balanced',
				agent_verbosity: verbosity ?? 'brief',
				agent_role: role?.trim() ?? '',
				agent_directive: directive?.trim() ?? '',
				activity: null,
				foreground_started: null,
				context_tokens: null,
				agent_unread: false,
				last_message_at: null,
				pinned: false,
				sort_order: Math.max(0, ...state.agents.map((a) => a.sort_order)) + 1,
				team_id: team_id ?? null,
				workspace_id: null,
				endpoint_model_id: null,
				following_count: 0
			};
			state.agents.push(created);
			return { ...created };
		},
		updateIntelligenceAgent: ({
			agent_id,
			name,
			color,
			emoji,
			personality,
			verbosity,
			role,
			directive
		}) => {
			const a = agent(agent_id);
			if (name != null) {
				if (name.trim() === '')
					throw new ProtocolError('validationFailed', 'Name can’t be empty.', 'name');
				a.agent_name = name;
			}
			if (color != null) a.agent_color = color;
			if (emoji != null) a.agent_emoji = emojiValue(emoji);
			if (personality != null) a.agent_personality = personality;
			if (verbosity != null) a.agent_verbosity = verbosity;
			if (role != null) a.agent_role = role.trim();
			if (directive != null) a.agent_directive = directive.trim();
			return agentRow(a);
		},
		deleteIntelligenceAgent: ({ agent_id }) => {
			const a = agent(agent_id);
			state.agents = state.agents.filter((x) => x.id !== agent_id);
			for (const t of state.tasks)
				if (t.assignee?.kind === 'agent' && t.assignee.agent_id === agent_id) t.assignee = null;
			state.agentMessages = state.agentMessages.filter((m) => m.agent_id !== agent_id);
			state.agentCues = state.agentCues.filter((c) => c.agent_id !== agent_id);
			state.agentAgenda = state.agentAgenda.filter((i) => i.agent_id !== agent_id);
			state.agentRuns = state.agentRuns.filter((r) => r.agent_id !== agent_id);
			state.memories = state.memories.filter(
				(m) => m.scope.kind !== 'agent' || m.scope.agent_id !== agent_id
			);
			state.agentSkills = state.agentSkills.filter(
				(s) => s.scope.kind !== 'agent' || s.scope.agent_id !== agent_id
			);
			return { ...a };
		},
		markAgentRead: ({ agent_id }) => {
			const a = agent(agent_id);
			a.agent_unread = false;
			return agentRow(a);
		},
		listAgentMessages: ({ agent_id, before, limit }) => {
			agent(agent_id);
			const older = state.agentMessages
				.filter((m) => m.agent_id === agent_id && (before == null || m.id < before))
				.sort((a, b) => a.id - b.id);
			return older.slice(Math.max(0, older.length - limit)).map((m) => ({ ...m }));
		},
		getAgentMessage: ({ message_id }) => ({ ...agentMessageById(message_id) }),
		setAgentMessageReaction: ({ message_id, reaction }) => {
			const m = agentMessageById(message_id);
			if (m.sender === 'user')
				throw new ProtocolError('validationFailed', 'React to the agent’s messages, not your own.');
			m.reaction = reaction?.trim() || null;
			m.reacted_at = nowSeconds();
			Object.assign(m, reactionExcerpt(m.reaction, m.excerpt));
			return null;
		},
		listAgentMessageAttachments: ({ message_ids }) =>
			state.agentAttachments
				.filter(
					(a) => a.message.kind === 'agent' && message_ids.includes(a.message.agent_message_id)
				)
				.map((a) => ({ ...a })),
		// The agent answers with a canned reply; it runs no code here. The
		// reply is one model call, on a thread of its own.
		sendAgentMessage: ({ agent_id, message, attachments, forward, skill, workspace_id }) => {
			// Like the daemon, the message records a workspace the agent wasn't
			// last told.
			const switched = workspace_id != null ? workspace(workspace_id) : null;
			const told = switched && switched.id !== agent(agent_id).workspace_id ? switched : null;
			if (told) agent(agent_id).workspace_id = told.id;
			// The forwarded item, live, as the daemon looks it up.
			let content: AgentMessageContent = { kind: 'text', text: message };
			if (forward && skill != null)
				throw new ProtocolError(
					'validationFailed',
					'A message can forward an item or use a skill, not both.'
				);
			if (skill != null) {
				// One of the agent's own or its team's, as the daemon checks.
				const a = agent(agent_id);
				const found = state.agentSkills.find(
					(s) =>
						s.id === skill &&
						((s.scope.kind === 'agent' && s.scope.agent_id === a.id) ||
							(s.scope.kind === 'team' && s.scope.team_id === a.team_id))
				);
				if (!found)
					throw new ProtocolError(
						'validationFailed',
						'This agent doesn’t have that skill.',
						'skill'
					);
				content = { kind: 'skill', name: found.name, prompt: found.prompt, note: message.trim() };
			} else if (forward?.kind === 'task') {
				const { title } = task(forward.workspace_id, forward.id);
				content = { kind: 'forward', note: message.trim(), item: { ...forward, title } };
			} else if (forward) {
				const { title, document_type } = document(forward.workspace_id, forward.id);
				const kind = document_type === 'journal' ? 'journal' : 'document';
				content = { kind: 'forward', note: message.trim(), item: { ...forward, kind, title } };
			}
			// Like the daemon's trigger: the agent's reply leaves it unread.
			agent(agent_id).agent_unread = true;
			const now = nowSeconds();
			const thread_id = nextId(state.threads);
			state.threads.push({
				id: thread_id,
				created_at: now,
				prompt_id: 1,
				system_prompt: '',
				origin: { kind: 'root' },
				remote: null
			});
			state.costEvents.push(costEvent(nextId(state.costEvents), thread_id, now, 6_000, 120));
			const user = agentMessage(nextId(state.agentMessages), agent_id, now, 'user', content, {
				attachment_count: attachments?.length ?? 0,
				switched_workspace_id: told?.id ?? null,
				switched_workspace_title: told?.title ?? null
			});
			state.agentMessages.push(user);
			for (const { media_type, data } of attachments ?? [])
				state.agentAttachments.push({
					id: nextId(state.agentAttachments),
					message: { kind: 'agent', agent_message_id: user.id },
					media_type,
					data
				});
			const reply = agentMessage(
				nextId(state.agentMessages),
				agent_id,
				now,
				'agent',
				`On it: ${message}`
			);
			state.agentMessages.push(reply);
			return null;
		},
		// The mock's turns end as they start, so there is never one to stop.
		stopAgentTurn: () => null,
		// The pick is recorded on the choice, and the agent's canned reply
		// follows; no message of the user's is posted.
		respondToAgentChoice: ({ message_id, option }) => {
			const m = agentMessageById(message_id);
			const choice = m.message_content;
			if (choice.kind !== 'choice')
				throw new ProtocolError('validationFailed', 'That message isn’t a choice.');
			if (choice.selected !== null)
				throw new ProtocolError('validationFailed', 'That choice has already been answered.');
			if (option >= choice.options.length)
				throw new ProtocolError('validationFailed', 'That choice has no such option.');
			choice.selected = option;
			agent(m.agent_id).agent_unread = true;
			const reply = agentMessage(
				nextId(state.agentMessages),
				m.agent_id,
				nowSeconds(),
				'agent',
				`Going with ${choice.options[option]}.`
			);
			state.agentMessages.push(reply);
			return null;
		},
		listAgentAgenda: ({ agent_id }) => {
			agent(agent_id);
			return state.agentAgenda
				.filter((i) => i.agent_id === agent_id)
				.sort((a, b) => a.position - b.position)
				.map((i) => ({ ...i }));
		},
		listAgentCues: ({ agent_id }) => {
			agent(agent_id);
			return state.agentCues.filter((c) => c.agent_id === agent_id).map((c) => ({ ...c }));
		},
		// What the filter picks: what an agent sees, the global ones, or every one; an agent's own
		// first, then a team's, then the global ones.
		listMemories: ({ filter }) => {
			const seer = typeof filter === 'object' ? agent(filter.seenBy) : null;
			const sees = ({ scope }: Memory) =>
				filter === 'every' ||
				scope.kind === 'global' ||
				(scope.kind === 'agent' && scope.agent_id === seer?.id) ||
				(scope.kind === 'team' && scope.team_id === seer?.team_id);
			const rank = { agent: 0, team: 1, global: 2 };
			return state.memories
				.filter(sees)
				.toSorted((a, b) => rank[a.scope.kind] - rank[b.scope.kind] || a.id - b.id)
				.map((m) => structuredClone(m));
		},
		forgetMemory: ({ memory_id }) => {
			const memory = state.memories.find((m) => m.id === memory_id) ?? notFound('memory');
			state.memories = state.memories.filter((m) => m.id !== memory_id);
			return { ...memory };
		},
		listAgentSkills: () => state.agentSkills.toSorted(byName).map((s) => structuredClone(s)),
		createAgentSkill: ({ scope, ...fields }) => {
			if (scope.kind === 'agent') agent(scope.agent_id);
			else agentTeam(scope.team_id);
			const created: AgentSkill = {
				id: nextId(state.agentSkills),
				scope,
				...skillFields(scope, fields),
				created_at: nowSeconds()
			};
			state.agentSkills.push(created);
			return structuredClone(created);
		},
		updateAgentSkill: ({ skill_id, ...fields }) => {
			const skill = state.agentSkills.find((s) => s.id === skill_id) ?? notFound('agent skill');
			Object.assign(skill, skillFields(skill.scope, fields, skill_id));
			return structuredClone(skill);
		},
		deleteAgentSkill: ({ skill_id }) => {
			const skill = state.agentSkills.find((s) => s.id === skill_id) ?? notFound('agent skill');
			state.agentSkills = state.agentSkills.filter((s) => s.id !== skill_id);
			return structuredClone(skill);
		},
		resetAgentAgenda: ({ agent_id }) => {
			agent(agent_id);
			const cleared = state.agentAgenda
				.filter((i) => i.agent_id === agent_id)
				.sort((a, b) => a.position - b.position);
			if (cleared.length === 0) return null;
			state.agentAgenda = state.agentAgenda.filter((i) => i.agent_id !== agent_id);
			state.agentMessages.push(
				agentMessage(nextId(state.agentMessages), agent_id, nowSeconds(), 'agent', {
					kind: 'agenda',
					change: 'reset',
					items: cleared.map((i) => i.text)
				})
			);
			return null;
		},
		removeAgentCue: ({ cue_id }) => {
			if (!state.agentCues.some((c) => c.id === cue_id)) notFound('intelligence agent cue');
			state.agentCues = state.agentCues.filter((c) => c.id !== cue_id);
			return null;
		},
		listActivities: ({ limit }) => {
			const running = state.activities
				.filter((a) => a.ended_at === null)
				.sort((a, b) => b.started_at - a.started_at);
			const ended = state.activities
				.filter((a) => a.ended_at !== null)
				.sort((a, b) => b.ended_at! - a.ended_at!)
				.slice(0, limit);
			return [...running, ...ended].map((a) => structuredClone(a));
		},
		listAgentRuns: ({ agent_id, limit }) => {
			agent(agent_id);
			return state.agentRuns
				.filter((r) => r.agent_id === agent_id)
				.sort((a, b) => b.started_at - a.started_at)
				.slice(0, limit)
				.map((r) => ({ ...r }));
		}
	} satisfies Partial<Handlers>;
}
