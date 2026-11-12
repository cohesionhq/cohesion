// Actors and their sessions.

import { ProtocolError } from '../../errors';
import type {
	ActorSession,
	ActorSessionMessage,
	ActorSessionMessageContent,
	ActorSessionSearchHit
} from '../../types';
import { nowSeconds, searchWords, starts } from '../support';
import type { Handlers, MockCore } from '../core';

export function actorsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		notFound,
		clientServes,
		actorSession,
		subagentsBelow,
		startedSession,
		attachActorImages
	} = core;
	return {
		listActors: () => state.actors.filter((a) => !a.deleted).map((a) => structuredClone(a)),
		// Both actors draw on one account, which their client names.
		listActorUsage: () => [
			{
				actor_ids: [1, 2],
				pool: { id: 'claude:me@example.com', name: 'me@example.com' },
				plan: [
					{ label: '5-hour limit', used: 0.84, resets_at: nowSeconds() + 2 * 3600 },
					{ label: 'Weekly limit', used: 0.18, resets_at: nowSeconds() + 4 * 86_400 }
				]
			}
		],
		listActorSessions: ({ actor_id }) =>
			state.actorSessions
				.filter(
					(t) =>
						t.status !== 'resolved' &&
						t.subagent === null &&
						(actor_id == null || t.actor_id === actor_id)
				)
				.sort((a, b) => b.status_changed_at - a.status_changed_at || b.id - a.id)
				.map((t) => ({ ...t })),
		getActorSession: ({ id }) => structuredClone(actorSession(id)),
		listActorSubagents: ({ session_id }) => {
			actorSession(session_id);
			return state.actorSessions
				.filter((t) => t.subagent?.parent_session_id === session_id)
				.sort((a, b) => a.started_at - b.started_at || a.id - b.id)
				.map((t) => structuredClone(t));
		},
		listActorSessionFamily: ({ session_id }) => {
			let top = actorSession(session_id);
			while (top.subagent) top = actorSession(top.subagent.parent_session_id);
			const below = (id: number): ActorSession[] =>
				state.actorSessions
					.filter((t) => t.subagent?.parent_session_id === id)
					.sort((a, b) => a.started_at - b.started_at || a.id - b.id)
					.flatMap((t) => [t, ...below(t.id)]);
			return [top, ...below(top.id)].map((t) => structuredClone(t));
		},
		listUnansweredSubagentRequests: ({ session_id }) => {
			actorSession(session_id);
			const going = subagentsBelow(session_id)
				.filter((t) => t.status === 'working' || t.status === 'needsAttention')
				.map((t) => t.id);
			return state.actorSessionMessages
				.filter(
					(m) =>
						going.includes(m.session_id) &&
						m.content.sender === 'actor' &&
						['question', 'permission', 'plan'].includes(m.content.kind) &&
						!m.answered
				)
				.sort((a, b) => a.id - b.id)
				.map((m) => structuredClone(m));
		},
		// Every word in the title, or in one text message; a title match first.
		searchActorSessions: ({ query }) => {
			const words = searchWords(query);
			if (words.length === 0) return [];
			const matches = (text: string) => words.every(starts(text));
			const hits: ActorSessionSearchHit[] = [];
			for (const t of state.actorSessions) {
				if (t.subagent) continue;
				const hit = {
					id: t.id,
					actor_id: t.actor_id,
					title: t.title,
					status_changed_at: t.status_changed_at
				};
				if (matches(t.title)) {
					hits.push({ ...hit, excerpt: null });
					continue;
				}
				const said = state.actorSessionMessages.find(
					(m) => m.session_id === t.id && m.content.kind === 'text' && matches(m.content.text)
				);
				if (said?.content.kind === 'text') hits.push({ ...hit, excerpt: said.content.text });
			}
			return hits.sort((a, b) => Number(a.excerpt !== null) - Number(b.excerpt !== null));
		},
		markActorSessionRead: ({ session_id }) => {
			const session = actorSession(session_id);
			session.unread = false;
			session.read_at = nowSeconds();
			return { ...session };
		},
		listActorSessionMessages: ({ session_id }) => {
			actorSession(session_id);
			return state.actorSessionMessages
				.filter((m) => m.session_id === session_id)
				.map((m) => structuredClone(m));
		},
		listActorSessionMessageAttachments: ({ message_id }) =>
			state.actorSessionAttachments
				.filter(
					(a) =>
						a.message.kind === 'actorSession' && a.message.actor_session_message_id === message_id
				)
				.map((a) => ({ ...a })),
		startActorSession: ({ actor_id, prompt, title, mode, model, effort, attachments }) => {
			const a = state.actors.find((x) => x.id === actor_id && !x.deleted) ?? notFound('actor');
			if (!clientServes(a.client_id))
				throw new ProtocolError('validationFailed', `${a.title} is offline.`);
			const text = prompt.trim();
			if (!text) throw new ProtocolError('validationFailed', 'Say what to do.', 'prompt');
			const now = nowSeconds();
			const session: ActorSession = {
				id: nextId(state.actorSessions),
				actor_id,
				title: title?.trim().split('\n')[0] || text.split('\n')[0],
				mode: mode ?? null,
				model: model ?? null,
				effort: effort ?? null,
				started_at: now,
				status_changed_at: now,
				status: 'working',
				unread: false,
				read_at: null,
				context: null,
				follower: null,
				subagent: null
			};
			state.actorSessions.push(session);
			const message: ActorSessionMessage = {
				id: nextId(state.actorSessionMessages),
				session_id: session.id,
				sent_at: now,
				content: { sender: 'user', kind: 'text', text },
				answered: false,
				attachment_count: attachments?.length ?? 0
			};
			state.actorSessionMessages.push(message);
			attachActorImages(message.id, attachments);
			return { ...session };
		},
		sendActorSessionMessage: ({ session_id, content, attachments }) => {
			// A message to a session that is done or failed picks it back up.
			const session = actorSession(session_id);
			const now = nowSeconds();
			if (session.subagent && !('request_id' in content))
				throw new ProtocolError(
					'validationFailed',
					'A subagent takes only responses to what it asked.'
				);
			if ('request_id' in content) {
				const request = state.actorSessionMessages.find((m) => m.id === content.request_id);
				if (request) request.answered = true;
			} else {
				// Words pass over every request still open in the session.
				for (const m of state.actorSessionMessages) {
					const { sender, kind } = m.content;
					if (
						m.session_id === session_id &&
						sender === 'actor' &&
						['question', 'permission', 'plan'].includes(kind)
					)
						m.answered = true;
				}
			}
			const message: ActorSessionMessage = {
				id: nextId(state.actorSessionMessages),
				session_id,
				sent_at: now,
				content: { sender: 'user', ...content } as ActorSessionMessageContent,
				answered: false,
				attachment_count: attachments?.length ?? 0
			};
			state.actorSessionMessages.push(message);
			attachActorImages(message.id, attachments);
			if (session.status !== 'working') {
				session.status = 'working';
				session.status_changed_at = now;
			}
			return structuredClone(message);
		},
		stopActorSession: ({ session_id }) => {
			const session = startedSession(session_id);
			if (session.status !== 'idle' && session.status !== 'resolved') {
				session.status = 'idle';
				session.status_changed_at = nowSeconds();
			}
			return { ...session };
		},
		resolveActorSession: ({ session_id }) => {
			const session = startedSession(session_id);
			if (session.status !== 'resolved') {
				session.status = 'resolved';
				session.status_changed_at = nowSeconds();
			}
			return { ...session };
		},
		unfollowActorSession: ({ session_id, agent_id }) => {
			const session = actorSession(session_id);
			if (session.follower?.agent_id === agent_id) session.follower = null;
			return { ...session };
		},
		setActorSessionMode: ({ session_id, mode }) => {
			const session = startedSession(session_id);
			session.mode = mode;
			return { ...session };
		}
	} satisfies Partial<Handlers>;
}
