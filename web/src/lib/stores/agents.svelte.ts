import { batched, groupBy } from '$lib/protocol/batch';
import { toProtocolError, type ProtocolError } from '$lib/protocol/errors';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import type {
	ImageUpload,
	AgentChoice,
	AgentSkill,
	AgentForwardTarget,
	AgentForwardedItem,
	HookKind,
	IntelligenceAgent,
	Attachment,
	IntelligenceAgentColor,
	AgentAgendaItem,
	IntelligenceAgentCue,
	IntelligenceAgentMessage,
	IntelligenceAgentPersonality,
	IntelligenceAgentRun,
	IntelligenceAgentTeam,
	IntelligenceAgentVerbosity,
	Invalidation,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

/** Messages per page of an agent's feed. */
export const FEED_PAGE = 50;
const RUNS_LIMIT = 50;

// ---- Messages ---------------------------------------------------------------

/** A feed message; `message_content.kind` says what it is. */
export type AgentMessage = IntelligenceAgentMessage;

/** An item a message forwards: as its stand-in shows it, and as the daemon is sent it. */
export interface Forward {
	item: AgentForwardedItem;
	target: AgentForwardTarget;
}

export interface AgentFeed {
	/** Oldest first. */
	messages: AgentMessage[];
	/** Whether messages older than the first one exist. */
	hasOlder: boolean;
}

// ---- Display ----------------------------------------------------------------

/** When a cue's hook fires, as a clause that finishes "When …". */
export function hookLabel(hook: HookKind): string {
	return vocabulary.hook(hook).cue_clause;
}

/**
 * What to call a cue. Cues migrated from an agent's single trigger are
 * named after their hook (`task:create`), so those read as the hook's
 * label instead.
 */
export function cueTitle(name: string, hook: HookKind): string {
	return name === hook ? `When ${hookLabel(hook)}` : name;
}

/**
 * A message in one line: its excerpt, after its verb when it has one
 * (`Noted: …`).
 */
export function messageLine(message: AgentMessage): string {
	return message.excerpt_verb ? `${message.excerpt_verb}: ${message.excerpt}` : message.excerpt;
}

// ---- Store ------------------------------------------------------------------

/** Whether a saved user message says what a stand-in did: the same text, forward or skill. */
function same(saved: AgentMessage['message_content'], standIn: AgentMessage['message_content']) {
	if (saved.kind === 'text' && standIn.kind === 'text') return saved.text === standIn.text;
	if (saved.kind === 'skill' && standIn.kind === 'skill')
		return saved.name === standIn.name && saved.note === standIn.note;
	if (saved.kind === 'forward' && standIn.kind === 'forward')
		return (
			saved.note === standIn.note &&
			saved.item.kind === standIn.item.kind &&
			saved.item.id === standIn.item.id
		);
	return false;
}

export interface AgentChanges {
	name?: string;
	color?: IntelligenceAgentColor;
	/** Blank clears it. */
	emoji?: string;
	personality?: IntelligenceAgentPersonality;
	verbosity?: IntelligenceAgentVerbosity;
	/** Blank clears it. */
	role?: string;
	/** Blank clears it. */
	directive?: string;
}

/** A new agent; the daemon picks a name when it has none, and defaults the rest. */
export interface NewAgent {
	name: string;
	color: IntelligenceAgentColor;
	emoji?: string | null;
	personality?: IntelligenceAgentPersonality;
	verbosity?: IntelligenceAgentVerbosity;
	role?: string;
	directive?: string;
	/** The team it starts on; none when absent. */
	team_id?: number | null;
}

let localId = 0;

/** Whether `key` is about agent `agent_id`. */
const forAgent = (key: Invalidation, agent_id: number) =>
	key.kind === 'agent' && key.id === agent_id;

export class AgentsStore {
	readonly list: Resource<IntelligenceAgent[]>;
	/** The teams agents are organized into, by title. */
	readonly teams: Resource<IntelligenceAgentTeam[]>;
	/** The selected agent's messages, newest page plus any older pages loaded. */
	readonly feed: Resource<AgentFeed, number>;
	/** The selected agent's cues, oldest first. */
	readonly cues: Resource<IntelligenceAgentCue[], number>;
	/** The selected agent's agenda, in order, finished items included. */
	readonly agenda: Resource<AgentAgendaItem[], number>;
	readonly runs: Resource<IntelligenceAgentRun[], number>;
	/** The agent the view shows. */
	selected = $state<number | null>(null);
	/**
	 * Per agent, stand-ins (negative ids) for the messages sent from here
	 * that the feed doesn't show saved yet, in the order sent.
	 */
	outbox = $state<Record<number, AgentMessage[]>>({});
	/**
	 * The images attached to each message fetched so far, by message id; a
	 * stand-in's are the ones being sent. They never change once saved.
	 */
	attachments = $state<Record<number, Attachment[]>>({});
	/** An older page is being fetched. */
	loadingOlder = $state(false);
	/**
	 * For each stand-in sent to the daemon, the newest id in the feed then:
	 * a user message above it is the saved copy. Null when the feed wasn't
	 * shown, so a user message with the same text stands for it. Local
	 * bookkeeping; the outbox is what renders.
	 */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#after = new Map<number, number | null>();
	/** Messages whose attachments are being fetched. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#fetching = new Set<number>();
	/** Per agent, settles with why the last message sent failed, or null once the daemon accepted it. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#tails = new Map<number, Promise<ProtocolError | null>>();
	/**
	 * Called when a refetch finds an agent newly unread while its
	 * conversation isn't the one shown; not for agents already unread on the
	 * first load.
	 */
	onNewMessage: ((agent: IntelligenceAgent) => void) | null = null;

	/** Each message's images; the messages on screen ask together, as one request. */
	#attachmentsOf: (message_id: number) => Promise<Attachment[]>;
	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.#attachmentsOf = batched(
			async (message_ids) =>
				groupBy(await send('listAgentMessageAttachments', { message_ids }), (a) =>
					a.message.kind === 'agent' ? a.message.agent_message_id : 0
				),
			() => []
		);
		this.list = new Resource({
			fetch: async () => {
				const list = await send('listIntelligenceAgents');
				if (this.list.loaded) this.#noticeNewMessages(list);
				return list;
			},
			initial: [],
			// Removing a model puts the agents that ran on it back on the default,
			// and a new message can leave its agent unread.
			matches: (key) =>
				key.kind === 'agent' || key.kind === 'intelligence' || key.kind === 'agentMessage'
		});
		this.teams = new Resource({
			fetch: () => send('listAgentTeams'),
			initial: [],
			matches: (key) => key.kind === 'agentTeams'
		});
		this.feed = new Resource({
			// A refetch keeps as many messages as are loaded, so older pages
			// stay, and refetches mid-send show each script the agent runs as
			// it is saved.
			fetch: async (agent_id) => {
				const loaded = this.feed.key === agent_id ? this.feed.value.messages.length : 0;
				const limit = Math.max(FEED_PAGE, loaded);
				const messages = await send('listAgentMessages', { agent_id, limit });
				return {
					messages: this.#withStandIns(agent_id, messages),
					hasOlder: messages.length === limit
				};
			},
			initial: { messages: [], hasOlder: false },
			// A message added or changed refreshes just itself; the agent's other
			// state (`agent`) isn't in the feed.
			patch: {
				matches: (key, agent_id) => key.kind === 'agentMessage' && key.id.agent_id === agent_id,
				fetch: async (keys, agent_id) => {
					const ids = keys.flatMap((k) => (k.kind === 'agentMessage' ? [k.id.message_id] : []));
					const fresh = await send('listAgentMessages', {
						agent_id,
						ids,
						limit: ids.length
					});
					return (feed: AgentFeed) => {
						const kept = feed.messages.filter((m) => m.id > 0 && !fresh.some((f) => f.id === m.id));
						// One older than the pages loaded stays out until they reach it.
						const oldest = feed.hasOlder ? (kept[0]?.id ?? 0) : 0;
						const shown = fresh.filter((f) => f.id >= oldest);
						const saved = [...kept, ...shown].sort((a, b) => a.id - b.id);
						return { ...feed, messages: this.#withStandIns(agent_id, saved) };
					};
				}
			}
		});
		this.cues = new Resource({
			fetch: (agent_id) => send('listAgentCues', { agent_id }),
			initial: [],
			matches: forAgent
		});
		this.agenda = new Resource({
			fetch: (agent_id) => send('listAgentAgenda', { agent_id }),
			initial: [],
			matches: forAgent
		});
		this.runs = new Resource({
			fetch: (agent_id) => send('listAgentRuns', { agent_id, limit: RUNS_LIMIT }),
			initial: [],
			matches: forAgent
		});
	}

	/** Loads the agents and their teams. */
	/** The text of the newest message `agent_id` wrote, or `null` if its latest are not text. */
	async latestText(agent_id: number): Promise<string | null> {
		const recent = await this.#send('listAgentMessages', { agent_id, limit: 10 });
		for (const m of recent.toReversed()) {
			if (m.sender !== 'user' && m.message_content.kind === 'text') return m.message_content.text;
		}
		return null;
	}

	async loadList(): Promise<void> {
		await Promise.all([this.list.load(), this.teams.load()]);
	}

	/** The team with id `team_id`, if it is listed. */
	team(team_id: number | null): IntelligenceAgentTeam | null {
		return this.teams.value.find((t) => t.id === team_id) ?? null;
	}

	/**
	 * Marks agent `agent_id`'s conversation read; the dot clears at once.
	 * The reply's copy of the agent isn't used: its activity may already be
	 * older than the list's, which follows the agent's invalidations.
	 */
	async markRead(agent_id: number): Promise<void> {
		this.list.update((list) =>
			list.map((a) => (a.id === agent_id ? { ...a, agent_unread: false } : a))
		);
		await this.#send('markAgentRead', { agent_id });
	}

	/** The listed agent `id`, if loaded. */
	agent(id: number | null): IntelligenceAgent | null {
		return id === null ? null : (this.list.value.find((a) => a.id === id) ?? null);
	}

	/** Shows agent `agent_id`'s feed, cues and agenda, or nothing for null. */
	open(agent_id: number | null): Promise<void> {
		this.selected = agent_id;
		if (agent_id === null) {
			this.release();
			return Promise.resolve();
		}
		return Promise.all([
			this.feed.load(agent_id),
			this.cues.load(agent_id),
			this.agenda.load(agent_id)
		]).then(() => undefined);
	}

	/** Stops refetching the selected agent's feed, cues, agenda and runs. */
	release(): void {
		this.feed.release();
		this.cues.release();
		this.agenda.release();
		this.runs.release();
	}

	async loadOlder(): Promise<void> {
		const agent_id = this.feed.key;
		const first = this.feed.value.messages[0];
		if (agent_id === undefined || !first || !this.feed.value.hasOlder || this.loadingOlder) return;
		this.loadingOlder = true;
		try {
			const older = await this.#send('listAgentMessages', {
				agent_id,
				before: first.id,
				limit: FEED_PAGE
			});
			if (this.feed.key !== agent_id) return;
			this.feed.update((feed) => ({
				messages: [...older, ...feed.messages],
				hasOlder: older.length === FEED_PAGE
			}));
		} finally {
			this.loadingOlder = false;
		}
	}

	/**
	 * Fetches the images attached to `message`, once; `attachments` holds
	 * them when they arrive. A failed fetch is tried again next call.
	 */
	async loadAttachments(message: AgentMessage): Promise<void> {
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
	 * Reacts to the agent's `message` with `reaction`, or clears it with
	 * null. The reaction shows at once; a failed one is taken back. The
	 * daemon queues the agent's turn on it, whose messages arrive with the
	 * feed's refetches on the agent's invalidations.
	 */
	async react(message: AgentMessage, reaction: string | null): Promise<void> {
		this.#replace({ ...message, reaction });
		try {
			await this.#send('setAgentMessageReaction', { message_id: message.id, reaction });
		} catch (error) {
			this.#replace(message);
			throw error;
		}
	}

	/**
	 * Sends `text`, with `images` attached and forwarding `forward`'s item or
	 * running `skill` if given, to agent `agent_id`; with a skill, `text` is
	 * the note. A stand-in
	 * shows at once, until the feed shows the saved message. Messages go out
	 * one at a time, each once the daemon accepted the one before, so they
	 * keep their order; the daemon hands each to the turn the agent is
	 * running, or queues the agent's turn on it. What
	 * the turns post arrives with the feed's refetches on the agent's
	 * invalidations, and its activity says when it works. When a send fails,
	 * the messages queued behind it are dropped with the same error, which
	 * each caller shows where it sent from.
	 */
	sendMessage(
		agent_id: number,
		text: string,
		images: ImageUpload[] = [],
		forward?: Forward,
		skill?: AgentSkill
	): Promise<void> {
		const local: AgentMessage = {
			id: --localId,
			agent_id,
			run_id: null,
			run_description: null,
			run_part: null,
			sent_at: Math.floor(Date.now() / 1000),
			sender: 'user',
			reaction: null,
			reacted_at: null,
			attachment_count: images.length,
			reply_to: null,
			switched_workspace_id: null,
			switched_workspace_title: null,
			message_content: forward
				? { kind: 'forward', note: text, item: forward.item }
				: skill
					? { kind: 'skill', name: skill.name, prompt: skill.prompt, note: text }
					: { kind: 'text', text },
			// A stand-in is never quoted: replies and reactions are to saved messages.
			excerpt: '',
			excerpt_verb: null,
			reaction_excerpt: null,
			reaction_excerpt_verb: null
		};
		if (images.length > 0)
			this.attachments[local.id] = images.map((image, index) => ({
				...image,
				id: -index - 1,
				message: { kind: 'agent', agent_message_id: local.id }
			}));
		const waiting = this.outbox[agent_id] ?? [];
		this.outbox[agent_id] = [...waiting, local];
		if (this.feed.key === agent_id)
			this.feed.update((feed) => ({ ...feed, messages: [...feed.messages, local] }));

		const ahead = waiting.length > 0 ? this.#tails.get(agent_id) : undefined;
		const sent = (async () => {
			const failed = ahead ? await ahead : null;
			if (failed) {
				this.#withdraw(local);
				throw failed;
			}
			await this.#deliver(local, text, images, forward?.target, skill?.id);
		})();
		this.#tails.set(
			agent_id,
			sent.then(
				() => null,
				(error: unknown) => toProtocolError(error)
			)
		);
		return sent;
	}

	/** Sends `text`, `images` and the item `forward` names or the skill `skill` for the stand-in `local`. */
	async #deliver(
		local: AgentMessage,
		text: string,
		images: ImageUpload[],
		forward: AgentForwardTarget | undefined,
		skill: number | undefined
	): Promise<void> {
		const { agent_id } = local;
		const shown = () => this.feed.key === agent_id;
		this.#after.set(
			local.id,
			shown()
				? Math.max(0, ...this.feed.value.messages.filter((m) => m.id > 0).map((m) => m.id))
				: null
		);
		try {
			await this.#send('sendAgentMessage', {
				agent_id,
				message: text,
				attachments: images.length > 0 ? images : null,
				forward: forward ?? null,
				skill: skill ?? null
			});
		} catch (error) {
			this.#withdraw(local);
			throw toProtocolError(error);
		}
		// The daemon saved the message before accepting it, so the refetch
		// shows the saved copy in place of the stand-in.
		try {
			if (shown()) await this.feed.refresh();
		} finally {
			this.#settle(local);
		}
	}

	/**
	 * Stops agent `agent_id`'s running turn; the agent's invalidation says
	 * once it has ended.
	 */
	async stopTurn(agent_id: number): Promise<void> {
		await this.#send('stopAgentTurn', { agent_id });
	}

	/**
	 * Picks option `option` of the choice in `message`, which shows the pick
	 * at once. The daemon records it and queues the agent's turn on it,
	 * whose messages arrive with the feed's refetches. The pick posts no
	 * message; a failed one is taken back.
	 */
	async pick(message: AgentMessage, choice: AgentChoice, option: number): Promise<void> {
		this.#replace({
			...message,
			message_content: { ...choice, kind: 'choice', selected: option }
		});
		try {
			await this.#send('respondToAgentChoice', { message_id: message.id, option });
		} catch (error) {
			this.#replace(message);
			throw error;
		}
	}

	/** Removes cue `cue_id`, so its hook stops waking the agent. */
	async removeCue(cue_id: number): Promise<void> {
		await this.#send('removeAgentCue', { cue_id });
		await Promise.all([this.cues.refetch(), this.list.refetch()]);
	}

	/** Clears agent `agent_id`'s agenda; it hears that the agenda was cleared. */
	async resetAgenda(agent_id: number): Promise<void> {
		await this.#send('resetAgentAgenda', { agent_id });
		await this.agenda.refetch();
	}

	async create({ name, color, emoji, personality, verbosity, role, directive, team_id }: NewAgent) {
		const created = await this.#send('createIntelligenceAgent', {
			name: name.trim() || null,
			color,
			emoji: emoji ?? null,
			personality: personality ?? null,
			verbosity: verbosity ?? null,
			role: role ?? null,
			directive: directive ?? null,
			team_id: team_id ?? null
		});
		await this.list.refetch();
		return created;
	}

	/**
	 * Puts the pinned agents in the order of `agent_ids`, showing it before
	 * the daemon confirms; the rest stay where they are, after them.
	 */
	async reorder(agent_ids: number[]): Promise<void> {
		this.list.update((list) => [
			...agent_ids.map((id) => list.find((a) => a.id === id)!),
			...list.filter((a) => !a.pinned)
		]);
		try {
			this.list.set(await this.#send('reorderIntelligenceAgents', { agent_ids }));
		} catch (error) {
			void this.list.refetch();
			throw error;
		}
	}

	/** Pins or unpins agent `agent_id`; the list's order is the daemon's. */
	async setPinned(agent_id: number, pinned: boolean): Promise<void> {
		await this.#send('setAgentPinned', { agent_id, pinned });
		await this.list.refetch();
	}

	/** Puts agent `agent_id` on team `team_id`, or on none; its pin stays as it was. */
	async setTeam(agent_id: number, team_id: number | null): Promise<IntelligenceAgent> {
		const moved = await this.#send('setAgentTeam', { agent_id, team_id });
		this.list.update((list) =>
			list.map((a) => (a.id === moved.id ? { ...a, team_id: moved.team_id } : a))
		);
		return moved;
	}

	/** Runs agent `agent_id` on endpoint model `endpoint_model_id`, or on the `agent` tier's default. */
	async setModel(agent_id: number, endpoint_model_id: number | null): Promise<IntelligenceAgent> {
		const set = await this.#send('setAgentModel', { agent_id, endpoint_model_id });
		this.list.update((list) =>
			list.map((a) => (a.id === set.id ? { ...a, endpoint_model_id: set.endpoint_model_id } : a))
		);
		return set;
	}

	/** Creates a team, with no agents on it yet. */
	async createTeam(title: string): Promise<IntelligenceAgentTeam> {
		const created = await this.#send('createAgentTeam', { title: title.trim() });
		this.teams.update((teams) => byTitle([...teams, created]));
		return created;
	}

	async renameTeam(team_id: number, title: string): Promise<IntelligenceAgentTeam> {
		const renamed = await this.#send('renameAgentTeam', { team_id, title: title.trim() });
		this.teams.update((teams) => byTitle(teams.map((t) => (t.id === renamed.id ? renamed : t))));
		return renamed;
	}

	/** Deletes a team; its agents stay, on no team, and its memories are forgotten. */
	async deleteTeam(team_id: number): Promise<IntelligenceAgentTeam> {
		const deleted = await this.#send('deleteAgentTeam', { team_id });
		this.teams.update((teams) => teams.filter((t) => t.id !== deleted.id));
		this.list.update((list) =>
			list.map((a) => (a.team_id === deleted.id ? { ...a, team_id: null } : a))
		);
		return deleted;
	}

	async update(agent_id: number, changes: AgentChanges): Promise<IntelligenceAgent> {
		const updated = await this.#send('updateIntelligenceAgent', { agent_id, ...changes });
		this.#applyEdit(updated);
		return updated;
	}

	async delete(agent_id: number): Promise<IntelligenceAgent> {
		const deleted = await this.#send('deleteIntelligenceAgent', { agent_id });
		this.list.update((list) => list.filter((a) => a.id !== deleted.id));
		return deleted;
	}

	/**
	 * `messages` with the agent's outbox after them, less any the daemon has
	 * saved, whose copies take over their images.
	 */
	#withStandIns(agent_id: number, messages: AgentMessage[]): AgentMessage[] {
		const unsaved = (this.outbox[agent_id] ?? []).filter((standIn) => {
			const saved = this.#saved(standIn, messages);
			const images = this.attachments[standIn.id];
			if (saved && images && !this.attachments[saved.id])
				this.attachments[saved.id] = images.map((image) => ({ ...image, message_id: saved.id }));
			return !saved;
		});
		return [...messages, ...unsaved];
	}

	/** The daemon's copy of the stand-in in `messages`, if there. */
	#saved(standIn: AgentMessage, messages: AgentMessage[]): AgentMessage | undefined {
		// Not sent yet.
		const after = this.#after.get(standIn.id);
		if (after === undefined) return undefined;
		return messages.find(
			(m) =>
				m.sender === 'user' &&
				m.id > 0 &&
				(after === null ? same(m.message_content, standIn.message_content) : m.id > after)
		);
	}

	/** Takes an unsent or failed stand-in out of its agent's outbox and feed. */
	#withdraw(local: AgentMessage): void {
		this.#settle(local);
		if (this.feed.key === local.agent_id)
			this.feed.update((feed) => ({
				...feed,
				messages: feed.messages.filter((m) => m.id !== local.id)
			}));
	}

	/** Takes `local` out of its agent's outbox; a saved copy replaces it on the next refetch. */
	#settle(local: AgentMessage): void {
		this.#after.delete(local.id);
		delete this.attachments[local.id];
		const rest = (this.outbox[local.agent_id] ?? []).filter((m) => m.id !== local.id);
		if (rest.length > 0) this.outbox[local.agent_id] = rest;
		else delete this.outbox[local.agent_id];
	}

	#replace(message: AgentMessage): void {
		if (this.feed.key !== message.agent_id) return;
		this.feed.update((feed) => ({
			...feed,
			messages: feed.messages.map((m) => (m.id === message.id ? message : m))
		}));
	}

	#noticeNewMessages(list: IntelligenceAgent[]): void {
		// A local lookup; nothing renders from it.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		const wasUnread = new Set(this.list.value.filter((a) => a.agent_unread).map((a) => a.id));
		for (const agent of list) {
			if (agent.agent_unread && !wasUnread.has(agent.id) && agent.id !== this.selected) {
				this.onNewMessage?.(agent);
			}
		}
	}

	/**
	 * Shows the fields the user edited from `agent`, a reply. Its activity
	 * is left to the list's refetches, which follow the agent's
	 * invalidations: a reply's copy may already be older than the list's.
	 */
	#applyEdit(agent: IntelligenceAgent): void {
		const {
			agent_name,
			agent_color,
			agent_emoji,
			agent_personality,
			agent_verbosity,
			agent_role,
			agent_directive
		} = agent;
		const edited = {
			agent_name,
			agent_color,
			agent_emoji,
			agent_personality,
			agent_verbosity,
			agent_role,
			agent_directive
		};
		this.list.update((list) => list.map((a) => (a.id === agent.id ? { ...a, ...edited } : a)));
	}
}

/** `teams` in the daemon's order, by title ignoring case. */
const byTitle = (teams: IntelligenceAgentTeam[]) =>
	teams.toSorted((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));

export function createAgentsStore(send: Send = defaultSend): AgentsStore {
	return new AgentsStore(send);
}
