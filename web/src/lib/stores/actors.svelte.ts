// Actors: the agents clients offer on their devices, the sessions handed to
// them, and what is said in each. Whether an actor can be reached is its
// client's online state, which the clients list says.

import { batched, groupBy } from '$lib/protocol/batch';
import { nowSeconds } from '$lib/format/dates';
import { send as defaultSend } from '$lib/protocol/api';
import type {
	Actor,
	ActorSession,
	Attachment,
	ActorSessionMessage,
	ActorUserContent,
	ImageUpload,
	ListedActorUsage,
	Send,
	StartActorSessionRequest
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

/** Whether `session` is going on: working, or waiting on the user. */
export function isGoing(session: ActorSession): boolean {
	return session.status === 'working' || session.status === 'needsAttention';
}

/**
 * Where `session` falls in the sidebar: waiting on the user, then working,
 * then done and unread, then done and read.
 */
function sidebarRank(session: ActorSession): number {
	if (session.status === 'needsAttention') return 0;
	if (session.status === 'working') return 1;
	return session.unread ? 2 : 3;
}

/** What a new session was last set to run on, picked in the composer. */
export interface ActorPick {
	actor_id: number;
	/** The model's identifier, or '' for the actor's default. */
	model: string;
	/** The effort's identifier, or '' for the model's default. */
	effort: string;
}

const PICK_KEY = 'cohesion.actorPick';

/** The actor, model and effort last picked for a new session, in this browser; none at first. */
export function rememberedPick(): ActorPick | null {
	try {
		const stored = JSON.parse(localStorage.getItem(PICK_KEY) ?? 'null') as Partial<ActorPick>;
		if (stored && typeof stored.actor_id === 'number') {
			return {
				actor_id: stored.actor_id,
				model: typeof stored.model === 'string' ? stored.model : '',
				effort: typeof stored.effort === 'string' ? stored.effort : ''
			};
		}
	} catch {
		// Fall through to none.
	}
	return null;
}

export function rememberPick(pick: ActorPick): void {
	try {
		localStorage.setItem(PICK_KEY, JSON.stringify(pick));
	} catch {
		// Remembering the pick is a convenience; ignore blocked storage.
	}
}

export class ActorsStore {
	/** Every actor the clients offer. */
	readonly list: Resource<Actor[]>;
	/** Every actor session not yet resolved, latest status change first. */
	readonly sessions: Resource<ActorSession[]>;
	/** One session, by id. */
	readonly session: Resource<ActorSession | null, number>;
	/** What was said in one session, by its id, oldest first. */
	readonly messages: Resource<ActorSessionMessage[], number>;
	/** The subagents one session's actor launched from it, one level down, by its id, first launched first. */
	readonly subagents: Resource<ActorSession[], number>;
	/** The requests waiting on the user in one session's subagents, at any depth, by its id, oldest first. */
	readonly waiting: Resource<ActorSessionMessage[], number>;
	/** The session one level above a subagent, by its id, for the way back up. */
	readonly above: Resource<ActorSession | null, number>;
	/** How much of its limits each actor whose client is connected has used, as the client said. */
	readonly usage: Resource<ListedActorUsage[]>;
	/**
	 * The images that came with each message, fetched so far, by
	 * message id. They never change once sent.
	 */
	attachments = $state<Record<number, Attachment[]>>({});

	#send: Send;
	/** Each message's images; the messages on screen ask together, as one request. */
	#attachmentsOf: (message_id: number) => Promise<Attachment[]>;
	/** Messages whose attachments are being fetched. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#fetching = new Set<number>();

	constructor(send: Send) {
		this.#send = send;
		this.#attachmentsOf = batched(
			async (message_ids) =>
				groupBy(await send('listActorSessionMessageAttachments', { message_ids }), (a) =>
					a.message.kind === 'actorSession' ? a.message.actor_session_message_id : 0
				),
			() => []
		);
		this.list = new Resource({
			fetch: () => send('listActors'),
			initial: [],
			matches: (key) => key.kind === 'actors'
		});
		this.sessions = new Resource({
			fetch: () => send('listActorSessions', {}),
			initial: [],
			matches: (key) => key.kind === 'actorSessions'
		});
		this.session = new Resource({
			fetch: (id) => send('getActorSession', { id }),
			initial: null,
			matches: (key, id) =>
				key.kind === 'actorSessions' || (key.kind === 'actorSession' && key.id === id)
		});
		this.messages = new Resource({
			fetch: (session_id) => send('listActorSessionMessages', { session_id }),
			initial: [],
			matches: (key, id) => key.kind === 'actorSession' && key.id === id
		});
		this.subagents = new Resource({
			fetch: (session_id) => send('listActorSubagents', { session_id }),
			initial: [],
			matches: (key, id) => key.kind === 'actorSession' && key.id === id
		});
		this.waiting = new Resource({
			fetch: (session_id) => send('listUnansweredSubagentRequests', { session_id }),
			initial: [],
			matches: (key, id) => key.kind === 'actorSession' && key.id === id
		});
		this.above = new Resource({
			fetch: (id) => send('getActorSession', { id }),
			initial: null,
			matches: (key, id) =>
				key.kind === 'actorSessions' || (key.kind === 'actorSession' && key.id === id)
		});
		this.usage = new Resource({
			fetch: () => send('listActorUsage'),
			initial: [],
			matches: (key) => key.kind === 'actorUsage'
		});
	}

	load(): Promise<void> {
		return Promise.all([this.list.load(), this.sessions.load()]).then(() => {});
	}

	/**
	 * Every session not yet resolved, those waiting on the user first, then
	 * those working, then those done and unread, then those done and read,
	 * each latest status change first. The sidebar shows the ones going on.
	 */
	get current(): ActorSession[] {
		return this.sessions.value.toSorted((a, b) => sidebarRank(a) - sidebarRank(b));
	}

	/**
	 * The sessions in `current` that agent `agent_id` follows, having handed
	 * them work, in the same order.
	 */
	followedBy(agent_id: number): ActorSession[] {
		return this.current.filter((session) => session.follower?.agent_id === agent_id);
	}

	/** How many sessions are going on, which the sidebar's rail counts. */
	get goingCount(): number {
		return this.sessions.value.filter(isGoing).length;
	}

	/** Marks session `session_id` read; the dot clears at once. */
	async markRead(session_id: number): Promise<void> {
		const read_at = nowSeconds();
		const read = (session: ActorSession) =>
			session.id === session_id ? { ...session, unread: false, read_at } : session;
		this.sessions.update((list) => list.map(read));
		if (this.session.key === session_id && this.session.value)
			this.session.set(read(this.session.value));
		await this.#send('markActorSessionRead', { session_id });
	}

	/** The session Cohesion started that `session` is, or runs under as a subagent at any depth. */
	async topOf(session: ActorSession): Promise<ActorSession> {
		if (!session.subagent) return session;
		const above = await this.#send('listActorSessionAncestors', { session_id: session.id });
		return above.at(-1) ?? session;
	}

	/** Hands an actor a new session, with any images attached to its prompt; its client must be online. */
	async start(request: StartActorSessionRequest): Promise<ActorSession> {
		const session = await this.#send('startActorSession', request);
		this.sessions.update((list) => [session, ...list.filter((t) => t.id !== session.id)]);
		return session;
	}

	/**
	 * Fetches the images attached to `message`, once; `attachments` holds
	 * them when they arrive. A failed fetch is tried again next call.
	 */
	async loadAttachments(message: ActorSessionMessage): Promise<void> {
		const { id } = message;
		if (message.attachment_count === 0 || this.attachments[id] || this.#fetching.has(id)) return;
		this.#fetching.add(id);
		try {
			this.attachments[id] = await this.#attachmentsOf(id);
		} finally {
			this.#fetching.delete(id);
		}
	}

	/**
	 * Says `content` in session `session_id`, with `images` attached: more to do, or
	 * a response to a request. A session that ended picks back up.
	 */
	async say(
		session_id: number,
		content: ActorUserContent,
		images: ImageUpload[] = []
	): Promise<ActorSessionMessage> {
		const message = await this.#send('sendActorSessionMessage', {
			session_id,
			content,
			attachments: images.length > 0 ? images : null
		});
		this.#keep(message.id, images);
		const request = 'request_id' in content ? content.request_id : null;
		if (this.messages.key === session_id) {
			this.messages.update((list) => [
				...list.map((m) => (m.id === request ? { ...m, answered: true } : m)),
				message
			]);
		}
		// A subagent's request answered from the session above leaves its list at once.
		if (this.waiting.value.some((m) => m.id === request))
			this.waiting.update((list) => list.filter((m) => m.id !== request));
		// Saying something starts it again, ended or not.
		if (this.session.key === session_id) void this.session.refetch();
		void this.sessions.refetch();
		return message;
	}

	/** Switches session `session_id` to mode `mode`, which its actor takes from its next step on. */
	async setMode(session_id: number, mode: string): Promise<ActorSession> {
		const session = await this.#send('setActorSessionMode', { session_id, mode });
		this.sessions.update((list) => list.map((t) => (t.id === session.id ? session : t)));
		if (this.session.key === session_id) this.session.set(session);
		return session;
	}

	/** Stops session `session_id` for good. */
	async stop(session_id: number): Promise<ActorSession> {
		const session = await this.#send('stopActorSession', { session_id });
		this.sessions.update((list) => list.map((t) => (t.id === session.id ? session : t)));
		if (this.session.key === session_id) this.session.set(session);
		return session;
	}

	/**
	 * Resolves session `session_id`, stopping it first if it is going on: it
	 * leaves the list at once, until something is said in it again.
	 */
	async resolve(session_id: number): Promise<ActorSession> {
		const session = await this.#send('resolveActorSession', { session_id });
		this.sessions.update((list) => list.filter((t) => t.id !== session.id));
		if (this.session.key === session_id) this.session.set(session);
		return session;
	}

	/**
	 * Has agent `agent_id` stop following session `session_id`: it no longer
	 * wakes when the session stops, and leaves the agent's list at once.
	 */
	async unfollow(session_id: number, agent_id: number): Promise<ActorSession> {
		const session = await this.#send('unfollowActorSession', { session_id, agent_id });
		this.sessions.update((list) => list.map((t) => (t.id === session.id ? session : t)));
		if (this.session.key === session_id) this.session.set(session);
		return session;
	}

	/** Holds the images just sent with message `message_id`, so they show without a fetch. */
	#keep(message_id: number, images: ImageUpload[]) {
		if (images.length === 0) return;
		this.attachments[message_id] = images.map((image, index) => ({
			...image,
			id: -(index + 1),
			message: { kind: 'actorSession', actor_session_message_id: message_id }
		}));
	}
}

export function createActorsStore(send: Send = defaultSend): ActorsStore {
	return new ActorsStore(send);
}
