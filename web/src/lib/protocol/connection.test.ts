import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Connection } from './connection.svelte';
import { ProtocolError } from './errors';
import { PROTOCOL_VERSION } from './types';

/** Just enough of `WebSocket` for `Connection`; tests drive the server side. */
class FakeSocket {
	static instances: FakeSocket[] = [];
	onmessage: ((event: { data: string }) => void) | null = null;
	onclose: (() => void) | null = null;
	/** Every frame sent, in order. */
	frames: { id: number; type: string; args?: unknown }[] = [];
	/** The requests sent: every frame but the subscribe. */
	sent: { id: number; type: string; args?: unknown }[] = [];

	constructor(readonly url: string) {
		FakeSocket.instances.push(this);
	}

	send(text: string) {
		const frame = JSON.parse(text);
		this.frames.push(frame);
		if (frame.type !== 'subscribe') this.sent.push(frame);
	}

	close() {
		this.onclose?.();
	}

	receive(frame: object) {
		this.onmessage?.({ data: JSON.stringify(frame) });
	}

	hello() {
		this.receive({ seq: 0, type: 'hello', protocol: PROTOCOL_VERSION, server: 'cohesiond/test' });
	}
}

function connect(onRefused = () => {}) {
	const connection = new Connection({
		url: () => 'ws://test/connect',
		WebSocket: FakeSocket as unknown as typeof WebSocket,
		onRefused
	});
	connection.connect();
	return { connection, socket: FakeSocket.instances.at(-1)! };
}

beforeEach(() => {
	FakeSocket.instances = [];
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('Connection', () => {
	it('holds requests until hello, then correlates replies by id', async () => {
		const { connection, socket } = connect();
		const first = connection.send('listWorkspaces');
		const second = connection.send('getJournal', { workspace_id: 2, date: '2026-10-13' });
		expect(socket.sent).toHaveLength(0);

		socket.hello();
		expect(connection.status).toBe('connected');
		expect(socket.sent.map((r) => r.type)).toEqual(['listWorkspaces', 'getJournal']);
		expect(socket.sent[0]).not.toHaveProperty('args');
		expect(socket.sent[1].args).toEqual({ workspace_id: 2, date: '2026-10-13' });

		// Answer out of order: each promise gets its own reply.
		const [a, b] = socket.sent;
		socket.receive({ seq: 1, type: 'reply', id: b.id, ok: { type: 'document', data: 'journal' } });
		socket.receive({ seq: 2, type: 'reply', id: a.id, ok: { type: 'workspaces', data: [] } });
		await expect(first).resolves.toEqual([]);
		await expect(second).resolves.toBe('journal');
	});

	it('rejects an error reply with a ProtocolError', async () => {
		const { connection, socket } = connect();
		socket.hello();
		const pending = connection.send('createWorkspace', { title: '' });
		socket.receive({
			seq: 1,
			type: 'reply',
			id: socket.sent[0].id,
			error: { code: 'validationFailed', message: 'A title is required.', field: 'title' }
		});
		const error = await pending.catch((e: unknown) => e);
		expect(error).toBeInstanceOf(ProtocolError);
		expect(error).toMatchObject({ code: 'validationFailed', field: 'title' });
	});

	it('resolves a raw request with its reply, an error reply included', async () => {
		const { connection, socket } = connect();
		socket.hello();
		const ok = connection.sendRaw({ type: 'listWorkspaces' });
		const refused = connection.sendRaw({ type: 'nope', args: { x: 1 } });
		const [a, b] = socket.sent;
		expect(b).toMatchObject({ type: 'nope', args: { x: 1 } });

		const error = { code: 'badRequest', message: 'Unknown request.' };
		socket.receive({ seq: 1, type: 'reply', id: b.id, error });
		socket.receive({ seq: 2, type: 'reply', id: a.id, ok: { type: 'workspaces', data: [] } });
		await expect(refused).resolves.toEqual({ error });
		await expect(ok).resolves.toEqual({ ok: { type: 'workspaces', data: [] } });
	});

	it('notifies reset and invalidate listeners', () => {
		const { connection, socket } = connect();
		socket.hello();
		const onReset = vi.fn();
		const onInvalidate = vi.fn();
		connection.onReset(onReset);
		connection.onInvalidate(onInvalidate);
		socket.receive({ seq: 1, type: 'reset' });
		socket.receive({ seq: 2, type: 'invalidate', keys: [{ kind: 'workspace', id: 3 }] });
		expect(onReset).toHaveBeenCalledOnce();
		expect(onInvalidate).toHaveBeenCalledWith([{ kind: 'workspace', id: 3 }]);
	});

	it('rejects pending requests on close and reconnects with backoff', async () => {
		const onRefused = vi.fn();
		const { connection, socket } = connect(onRefused);
		socket.hello();
		const onReconnected = vi.fn();
		connection.onReconnected(onReconnected);
		const pending = connection.send('listWorkspaces');

		socket.close();
		await expect(pending).rejects.toMatchObject({ code: 'disconnected' });
		expect(connection.status).toBe('reconnecting');
		expect(onRefused).not.toHaveBeenCalled();

		vi.advanceTimersByTime(499);
		expect(FakeSocket.instances).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(FakeSocket.instances).toHaveLength(2);

		// A second failure waits twice as long.
		FakeSocket.instances[1].close();
		vi.advanceTimersByTime(999);
		expect(FakeSocket.instances).toHaveLength(2);
		vi.advanceTimersByTime(1);
		expect(FakeSocket.instances).toHaveLength(3);

		FakeSocket.instances[2].hello();
		expect(connection.status).toBe('connected');
		expect(onReconnected).toHaveBeenCalledOnce();
	});

	it('subscribes to invalidations first on every greeted socket', async () => {
		const { connection, socket } = connect();
		const pending = connection.send('listWorkspaces');
		socket.hello();
		expect(socket.frames.map((f) => f.type)).toEqual(['subscribe', 'listWorkspaces']);

		socket.close();
		await expect(pending).rejects.toBeInstanceOf(ProtocolError);
		vi.advanceTimersByTime(500);
		const next = FakeSocket.instances[1];
		next.hello();
		expect(next.frames.map((f) => f.type)).toEqual(['subscribe']);
	});

	it('keeps requests made before hello across a socket that closes first', async () => {
		const { connection, socket } = connect();
		const pending = connection.send('listWorkspaces');
		socket.close();

		vi.advanceTimersByTime(500);
		const next = FakeSocket.instances[1];
		next.hello();
		expect(next.sent).toEqual([{ id: 1, type: 'listWorkspaces' }]);
		next.receive({ seq: 1, type: 'reply', id: 1, ok: { type: 'workspaces', data: [] } });
		await expect(pending).resolves.toEqual([]);
	});

	it('stops for good once the daemon revokes this client', async () => {
		const { connection, socket } = connect();
		socket.hello();
		const pending = connection.send('listWorkspaces');
		socket.receive({ seq: 1, type: 'evicted', reason: 'revoked' });
		socket.close();
		await expect(pending).rejects.toMatchObject({ code: 'disconnected' });
		expect(connection.fatal).toBe('This client was revoked. Pair it again to reconnect.');
		vi.advanceTimersByTime(10_000);
		expect(FakeSocket.instances).toHaveLength(1);
	});

	it('reports a socket closed before its first hello as refused', () => {
		const onRefused = vi.fn();
		const { socket } = connect(onRefused);
		socket.close();
		expect(onRefused).toHaveBeenCalledOnce();
	});
});
