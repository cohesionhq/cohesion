import { describe, expect, it } from 'vitest';
import { ProtocolError } from '$lib/protocol/errors';
import { createMock } from '$lib/protocol/mock';
import type { Send } from '$lib/protocol/types';
import { FEED_PAGE, createAgentsStore, cueTitle } from './agents.svelte';
import { refreshInvalidated } from './resource.svelte';

/** The agent's writes: a message, a pick, a proposal's answer, a cue's removal. */
const WRITES = new Set([
	'sendAgentMessage',
	'respondToAgentProposal',
	'respondToAgentChoice',
	'removeAgentCue'
]);

/** `send`, with the agent's writes handed to `writes` instead. */
function routed(send: Send, writes: Send): Send {
	return ((...args: Parameters<Send>) => (WRITES.has(args[0]) ? writes : send)(...args)) as Send;
}

/** The mock's `send`, held until `release()` so the pending state can be seen. */
function heldCall(send: Send) {
	let release = () => {};
	const gate = new Promise<void>((resolve) => (release = resolve));
	const call = (async (...args: Parameters<Send>) => {
		await gate;
		return send(...args);
	}) as Send;
	return { call, release };
}

describe('agents store', () => {
	it('opens an agent with its feed and its cues', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.loadList();
		await store.open(2);

		expect(store.cues.value.map((c) => c.hook)).toEqual(['task:create', 'task:priority']);
		const types = store.feed.value.messages.map((m) => m.message_content.kind);
		expect(types).toContain('codeExecution');
		const waiting = store.feed.value.messages.find(
			(m) => m.message_content.kind === 'choice' && m.message_content.selected === null
		);
		expect(waiting).toBeDefined();
		// A run's messages keep what woke it.
		expect(waiting?.run_description).toBe(
			'“Add a command to manually rebuild an agent’s context” was created'
		);
		expect(store.feed.value.hasOlder).toBe(false);

		await store.open(1);
		expect(store.cues.value).toEqual([]);
	});

	it('pages older messages in before the loaded ones', async () => {
		const mock = createMock();
		// Push agent 1 past two pages, after every seeded message.
		const base = Math.max(...mock.state.agentMessages.map((m) => m.id));
		for (let i = 0; i < FEED_PAGE * 2; i++) {
			mock.state.agentMessages.push({
				id: base + i + 1,
				agent_id: 1,
				run_id: null,
				run_description: null,
				run_part: null,
				sent_at: i,
				sender: 'user',
				message_content: { kind: 'text', text: `m${i}` },
				reaction: null,
				reacted_at: null,
				attachment_count: 0,
				reply_to: null,
				switched_workspace_id: null,
				switched_workspace_title: null,
				excerpt: `m${i}`,
				excerpt_verb: null,
				reaction_excerpt: null,
				reaction_excerpt_verb: null
			});
		}
		const total = mock.state.agentMessages.filter((m) => m.agent_id === 1).length;
		const store = createAgentsStore(mock.send);
		await store.open(1);
		expect(store.feed.value.messages).toHaveLength(FEED_PAGE);
		expect(store.feed.value.hasOlder).toBe(true);

		await store.loadOlder();
		await store.loadOlder();
		const ids = store.feed.value.messages.map((m) => m.id);
		expect(ids).toHaveLength(total);
		expect(ids).toEqual([...ids].sort((a, b) => a - b));
		expect(store.feed.value.hasOlder).toBe(false);

		// A background refetch keeps every loaded page.
		await store.feed.refetch();
		expect(store.feed.value.messages).toHaveLength(total);
	});

	it('shows the sent message at once, then the saved copy and what the turn posts', async () => {
		const mock = createMock();
		const { call, release } = heldCall(mock.send);
		const store = createAgentsStore(routed(mock.send, call));
		await store.open(1);
		const before = store.feed.value.messages.length;

		const sending = store.sendMessage(1, 'Watch my inbox');
		expect(store.outbox[1]).toMatchObject([
			{ message_content: { kind: 'text', text: 'Watch my inbox' } }
		]);
		expect(store.feed.value.messages).toHaveLength(before + 1);
		expect(store.feed.value.messages.at(-1)).toMatchObject({
			id: expect.any(Number),
			sender: 'user',
			message_content: { kind: 'text', text: 'Watch my inbox' }
		});

		release();
		await sending;
		// The daemon saved the message before accepting it; the refetch after
		// shows it, with what the (mock's instant) turn posted, in place of
		// the stand-in.
		expect(store.outbox[1]).toBeUndefined();
		const texts = store.feed.value.messages
			.slice(before)
			.map((m) =>
				m.message_content.kind === 'text' ? m.message_content.text : m.message_content.kind
			);
		expect(texts).toEqual(['Watch my inbox', 'On it: Watch my inbox']);
		expect(store.feed.value.messages.every((m) => m.id > 0)).toBe(true);
	});

	it('forwards an item the daemon names, and quotes it by its title without a note', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.open(1);
		const item = {
			kind: 'task',
			workspace_id: 1,
			id: 1,
			title: 'Configure and deploy the Linear connector'
		} as const;

		await store.sendMessage(1, '', [], {
			item,
			target: { kind: 'task', workspace_id: 1, id: 1 }
		});
		expect(store.outbox[1]).toBeUndefined();
		const saved = store.feed.value.messages.find((m) => m.message_content.kind === 'forward');
		expect(saved).toMatchObject({
			id: expect.any(Number),
			message_content: { kind: 'forward', note: '', item },
			excerpt: 'Configure and deploy the Linear connector',
			excerpt_verb: null
		});
	});

	it('shows attached images on the stand-in, then fetches the saved message’s', async () => {
		const mock = createMock();
		const { call, release } = heldCall(mock.send);
		const store = createAgentsStore(routed(mock.send, call));
		await store.open(1);
		const image = { media_type: 'image/png', data: 'aGk=' };

		const sending = store.sendMessage(1, '', [image]);
		const standIn = store.feed.value.messages.at(-1)!;
		expect(standIn.attachment_count).toBe(1);
		expect(store.attachments[standIn.id]).toMatchObject([image]);

		release();
		await sending;
		await store.feed.refetch();
		const saved = store.feed.value.messages.findLast((m) => m.sender === 'user')!;
		expect(saved.id).toBeGreaterThan(0);
		expect(saved.attachment_count).toBe(1);
		expect(store.attachments[standIn.id]).toBeUndefined();

		delete store.attachments[saved.id];
		await store.loadAttachments(saved);
		expect(store.attachments[saved.id]).toMatchObject([
			{ ...image, message: { kind: 'agent', agent_message_id: saved.id } }
		]);
	});

	it('reacts to the agent’s messages at once, taking back a failed reaction', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.open(1);
		const reply = store.feed.value.messages.find((m) => m.sender === 'agent')!;
		const own = store.feed.value.messages.find((m) => m.sender === 'user')!;

		await store.react(reply, '👍');
		await store.feed.refetch();
		expect(store.feed.value.messages.find((m) => m.id === reply.id)?.reaction).toBe('👍');
		await store.react(
			store.feed.value.messages.find((m) => m.id === reply.id)!,
			null
		);
		expect(store.feed.value.messages.find((m) => m.id === reply.id)?.reaction).toBeNull();

		await expect(store.react(own, '👍')).rejects.toBeInstanceOf(ProtocolError);
		expect(store.feed.value.messages.find((m) => m.id === own.id)?.reaction).toBe(own.reaction);
	});

	it('takes the stand-in back when sending fails', async () => {
		const mock = createMock();
		const failing = (async () => {
			throw new ProtocolError('badRequest', 'No model.');
		}) as Send;
		const store = createAgentsStore(routed(mock.send, failing));
		await store.open(1);
		const before = store.feed.value.messages.length;
		await expect(store.sendMessage(1, 'Hi')).rejects.toMatchObject({ message: 'No model.' });
		expect(store.outbox[1]).toBeUndefined();
		expect(store.feed.value.messages).toHaveLength(before);
	});

	it('marks a pick at once, posting no message of its own', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.open(1);
		const choice = store.feed.value.messages.find(
			(m) => m.message_content.kind === 'choice' && m.message_content.selected === null
		)!;
		if (choice.message_content.kind !== 'choice') throw new Error('expected a choice');
		const options = choice.message_content;
		const before = store.feed.value.messages.length;

		const picking = store.pick(choice, options, 1);
		expect(
			store.feed.value.messages.find((m) => m.id === choice.id)?.message_content
		).toMatchObject({
			selected: 1
		});
		await picking;
		expect(store.outbox[1]).toBeUndefined();
		await store.feed.refetch();
		const messages = store.feed.value.messages;
		expect(messages.find((m) => m.id === choice.id)?.message_content).toMatchObject({
			selected: 1
		});
		// Only what the turn posted is new.
		expect(messages.slice(before).map((m) => m.sender)).toEqual(['agent']);

		// A choice takes one pick.
		await expect(store.pick(choice, options, 0)).rejects.toMatchObject({
			message: 'That choice has already been answered.'
		});
	});

	it('offers a choice again when the pick fails', async () => {
		const mock = createMock();
		const failing = (async () => {
			throw new ProtocolError('badRequest', 'No model.');
		}) as Send;
		const store = createAgentsStore(routed(mock.send, failing));
		await store.open(1);
		const choice = store.feed.value.messages.find(
			(m) => m.message_content.kind === 'choice' && m.message_content.selected === null
		)!;
		if (choice.message_content.kind !== 'choice') throw new Error('expected a choice');
		const options = choice.message_content;

		await expect(store.pick(choice, options, 0)).rejects.toMatchObject({ message: 'No model.' });
		await store.feed.refetch();
		expect(
			store.feed.value.messages.find((m) => m.id === choice.id)?.message_content
		).toMatchObject({
			selected: null
		});
	});

	it('removes cues one at a time', async () => {
		const store = createAgentsStore(createMock().send);
		await store.loadList();
		await store.open(2);
		const [first, second] = store.cues.value;
		await store.removeCue(first.id);
		expect(store.cues.value.map((c) => c.id)).toEqual([second.id]);
		await store.removeCue(second.id);
		expect(store.cues.value).toEqual([]);
	});

	it('organizes agents into teams, keeping pins, and unteams them when one is deleted', async () => {
		const store = createAgentsStore(createMock().send);
		await store.loadList();
		const titles = () => store.teams.value.map((t) => t.title);
		expect(titles()).toEqual(['Assistants', 'Cohesion Engineers']);
		const field = await store.createTeam(' Field work ');
		expect(titles()).toEqual(['Assistants', 'Cohesion Engineers', 'Field work']);
		await expect(store.createTeam('assistants')).rejects.toMatchObject({
			code: 'validationFailed'
		});

		await store.setTeam(1, field.id);
		expect(store.agent(1)).toMatchObject({ team_id: field.id, pinned: true });
		await store.renameTeam(field.id, 'Outreach');
		expect(titles()).toEqual(['Assistants', 'Cohesion Engineers', 'Outreach']);
		expect(store.team(field.id)?.title).toBe('Outreach');

		await store.deleteTeam(field.id);
		expect(titles()).toEqual(['Assistants', 'Cohesion Engineers']);
		expect(store.agent(1)).toMatchObject({ team_id: null, pinned: true });
	});

	it('gives an agent a model of its own, and puts it back on the default when the model is removed', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.loadList();
		expect(store.agent(1)?.endpoint_model_id).toBeNull();

		await store.setModel(1, 3);
		expect(store.agent(1)?.endpoint_model_id).toBe(3);
		await expect(store.setModel(1, 999)).rejects.toMatchObject({ code: 'notFound' });

		await mock.send('removeEndpointModel', { id: 3 });
		await refreshInvalidated([{ kind: 'intelligence' }]);
		expect(store.agent(1)?.endpoint_model_id).toBeNull();
		store.list.release();
	});

	it('creates an agent on a team', async () => {
		const store = createAgentsStore(createMock().send);
		await store.loadList();
		const created = await store.create({ name: 'Lookout', color: 'teal', team_id: 2 });
		expect(store.agent(created.id)?.team_id).toBe(2);
	});

	it('lists pinned agents first in their order, then the rest newest first', async () => {
		const store = createAgentsStore(createMock().send);
		await store.loadList();
		const names = () => store.list.value.map((a) => a.agent_name);
		const created = await store.create({ name: 'Lookout', color: 'teal' });
		const rest = ['Secretary', 'Researcher', 'Plus Engineer'];
		expect(names()).toEqual(['Core Engineer', 'Lookout', ...rest]);
		await store.setPinned(created.id, true);
		expect(names()).toEqual(['Core Engineer', 'Lookout', ...rest]);
		await store.reorder([created.id, 1]);
		expect(names()).toEqual(['Lookout', 'Core Engineer', ...rest]);
		await store.setPinned(1, false);
		expect(names()).toEqual([
			'Lookout',
			'Secretary',
			'Core Engineer',
			'Researcher',
			'Plus Engineer'
		]);
	});

	it('creates, updates and deletes agents in the list', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.loadList();
		const created = await store.create({ name: ' Planner ', color: 'purple' });
		expect(created).toMatchObject({ agent_name: 'Planner', activity: null });
		expect(store.list.value.some((a) => a.id === created.id)).toBe(true);

		await store.update(created.id, { color: 'teal', emoji: ' 🌱 ' });
		expect(store.agent(created.id)).toMatchObject({ agent_color: 'teal', agent_emoji: '🌱' });

		await store.delete(created.id);
		expect(store.agent(created.id)).toBeNull();
	});

	it('refetches the list and only the touched agent on an agent invalidation', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		await store.loadList();
		await store.open(1);
		await store.runs.load(1);
		const before = store.feed.value.messages.length;
		mock.state.agents[0].agent_name = 'Renamed';
		mock.state.agentMessages.push({
			...mock.state.agentMessages.find((m) => m.agent_id === 1)!,
			id: 1000,
			message_content: { kind: 'text', text: 'pushed' }
		});

		await refreshInvalidated([{ kind: 'agent', id: 2 }]);
		expect(store.agent(1)?.agent_name).toBe('Renamed');
		expect(store.feed.value.messages).toHaveLength(before);
		expect([store.cues, store.runs].some((r) => r.touchedBy([{ kind: 'agent', id: 2 }]))).toBe(
			false
		);

		await refreshInvalidated([{ kind: 'agent', id: 1 }]);
		expect(store.feed.value.messages).toHaveLength(before + 1);
		expect(store.cues.touchedBy([{ kind: 'agent', id: 1 }])).toBe(true);
		expect(store.runs.touchedBy([{ kind: 'agent', id: 1 }])).toBe(true);
		store.list.release();
		store.release();
	});

	it('announces an agent newly unread unless its conversation is shown', async () => {
		const mock = createMock();
		const store = createAgentsStore(mock.send);
		const announced: string[] = [];
		store.onNewMessage = (agent) => announced.push(agent.agent_name);
		await store.loadList();
		// Unread before the first load: a dot, but no announcement.
		expect(store.agent(2)?.agent_unread).toBe(true);
		expect(announced).toEqual([]);

		await store.open(2);
		await store.markRead(2);
		expect(store.agent(2)?.agent_unread).toBe(false);
		expect(mock.state.agents[1].agent_unread).toBe(false);

		mock.state.agents.forEach((a) => (a.agent_unread = true));
		await store.list.refetch();
		expect(announced).toEqual(['Core Engineer', 'Researcher', 'Plus Engineer']);

		await store.list.refetch();
		expect(announced).toEqual(['Core Engineer', 'Researcher', 'Plus Engineer']);
		store.list.release();
		store.release();
	});

	it('keeps the stand-in until the feed shows the saved message', async () => {
		const mock = createMock();
		const { call, release } = heldCall(mock.send);
		const store = createAgentsStore(routed(mock.send, call));
		await store.open(1);
		const before = store.feed.value.messages.length;

		const sending = store.sendMessage(1, 'Hold on');
		await refreshInvalidated([{ kind: 'agent', id: 1 }]);
		expect(store.feed.value.messages).toHaveLength(before + 1);
		expect(store.feed.value.messages.at(-1)?.id).toBeLessThan(0);

		// The daemon saves the message, then the agent runs a script.
		const template = mock.state.agentMessages.find((m) => m.agent_id === 1)!;
		const saved = {
			...template,
			id: 1000,
			sender: 'user' as const,
			message_content: { kind: 'text' as const, text: 'Hold on' }
		};
		const execution = {
			title: 'Count',
			code: '1',
			outcome: { status: 'succeeded' as const, result: '1' }
		};
		const ran = {
			...template,
			id: 1001,
			sender: 'agent' as const,
			message_content: { kind: 'codeExecution' as const, ...execution }
		};
		mock.state.agentMessages.push(saved, ran);
		await refreshInvalidated([{ kind: 'agent', id: 1 }]);
		expect(store.feed.value.messages.map((m) => m.id).slice(-2)).toEqual([1000, 1001]);
		expect(store.feed.value.messages.every((m) => m.id > 0)).toBe(true);

		release();
		await sending;
		store.release();
	});

	it('sends a message only once the daemon accepted the one before', async () => {
		const mock = createMock();
		const { call, release } = heldCall(mock.send);
		const sent: string[] = [];
		const recording = (async (...args: Parameters<Send>) => {
			sent.push((args[1] as { message: string }).message);
			return call(...args);
		}) as Send;
		const store = createAgentsStore(routed(mock.send, recording));
		await store.open(1);
		const before = store.feed.value.messages.length;

		const first = store.sendMessage(1, 'One');
		const second = store.sendMessage(1, 'Two');
		await Promise.resolve();
		expect(sent).toEqual(['One']);
		const texts = () =>
			store.feed.value.messages
				.slice(before)
				.map((m) =>
					m.message_content.kind === 'text' ? m.message_content.text : m.message_content.kind
				);
		expect(texts()).toEqual(['One', 'Two']);

		release();
		await first;
		await second;
		expect(sent).toEqual(['One', 'Two']);
		await store.feed.refetch();
		expect(texts()).toEqual(['One', 'On it: One', 'Two', 'On it: Two']);
		expect(store.outbox[1]).toBeUndefined();
		store.release();
	});

	it('drops the messages queued behind a failed one', async () => {
		const mock = createMock();
		let fail = () => {};
		const gate = new Promise<void>((resolve) => (fail = resolve));
		const failing = (async () => {
			await gate;
			throw new ProtocolError('badRequest', 'No model.');
		}) as Send;
		const store = createAgentsStore(routed(mock.send, failing));
		await store.open(1);
		const before = store.feed.value.messages.length;

		const first = store.sendMessage(1, 'One');
		const second = store.sendMessage(1, 'Two');
		fail();
		await expect(first).rejects.toMatchObject({ message: 'No model.' });
		await expect(second).rejects.toMatchObject({ message: 'No model.' });
		expect(store.outbox[1]).toBeUndefined();
		expect(store.feed.value.messages).toHaveLength(before);
	});
});

describe('cueTitle', () => {
	it('reads a cue named after its hook as the hook', () => {
		expect(cueTitle('task:create', 'task:create')).toBe('When a task is created');
	});

	it('keeps a name the agent chose', () => {
		expect(cueTitle('File new tasks', 'task:create')).toBe('File new tasks');
	});
});
