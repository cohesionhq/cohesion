// The one WebSocket to the daemon. Requests are numbered and matched to
// replies by id; requests made before `hello` wait in an outbox, across
// reconnects, until a socket is greeted. Each greeted socket subscribes to
// invalidations before anything else, so no change after a load goes
// unheard. On close every request already sent rejects and the socket
// reconnects with backoff, unless the daemon ended it for good (this client
// was revoked).

import { ProtocolError } from './errors';
import {
	PROTOCOL_VERSION,
	type ArgsOf,
	type Frame,
	type Invalidation,
	type RawRequest,
	type Replies,
	type ReplyFrame,
	type RequestType,
	type WireRequest
} from './types';
import { vocabulary } from './vocabulary.svelte';

export type ConnectionStatus = 'connecting' | 'connected' | 'reconnecting';

export interface ConnectionOptions {
	/** Builds the socket URL for each attempt. */
	url: () => string;
	/** Injected for tests; defaults to the global `WebSocket`. */
	WebSocket?: typeof WebSocket;
	/** Called when the very first attempt closes before `hello` (a refused token). */
	onRefused?: () => void;
}

const MIN_BACKOFF = 500;
const MAX_BACKOFF = 10_000;

interface Pending {
	/** Takes the reply, an error reply included. */
	settle: (frame: ReplyFrame) => void;
	reject: (error: ProtocolError) => void;
}

export class Connection {
	status = $state<ConnectionStatus>('connecting');
	/** Set when the daemon speaks a protocol this UI doesn't, or revoked this client; no reconnects follow. */
	fatal = $state<string | null>(null);

	#options: ConnectionOptions;
	#socket: WebSocket | null = null;
	#nextId = 1;
	#pending = new Map<number, Pending>();
	/** Requests not yet sent, by id, in the order they were made. */
	#outbox = new Map<number, string>();
	#backoff = MIN_BACKOFF;
	#timer: ReturnType<typeof setTimeout> | null = null;
	#greeted = false;
	#everGreeted = false;
	#closed = false;
	#invalidateListeners = new Set<(keys: Invalidation[]) => void>();
	#resetListeners = new Set<() => void>();
	#connectedListeners = new Set<() => void>();

	constructor(options: ConnectionOptions) {
		this.#options = options;
	}

	connect(): void {
		this.#closed = false;
		const Impl = this.#options.WebSocket ?? WebSocket;
		const socket = new Impl(this.#options.url());
		this.#socket = socket;
		this.#greeted = false;
		socket.onmessage = (event) => this.#receive(JSON.parse(String(event.data)) as Frame);
		socket.onclose = () => this.#handleClose(socket);
	}

	/** Closes for good: no reconnect. */
	close(): void {
		this.#closed = true;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = null;
		this.#socket?.close();
	}

	send<K extends RequestType>(type: K, ...args: ArgsOf<K>): Promise<Replies[K]> {
		const request: WireRequest = { type };
		if (args.length > 0) request.args = args[0];
		return new Promise<Replies[K]>((resolve, reject) => {
			this.#request(request, {
				settle: (frame) => {
					if ('error' in frame) reject(ProtocolError.from(frame.error));
					else resolve(frame.ok.data as Replies[K]);
				},
				reject
			});
		});
	}

	/** Sends any request by name and resolves with its whole reply, error replies included. */
	sendRaw(request: RawRequest): Promise<ReplyFrame> {
		return new Promise<ReplyFrame>((resolve, reject) => {
			this.#request(request, { settle: resolve, reject });
		});
	}

	#request(request: RawRequest, pending: Pending): void {
		const id = this.#nextId++;
		const text = JSON.stringify({ id, ...request });
		this.#pending.set(id, pending);
		if (this.#greeted && this.#socket) this.#socket.send(text);
		else this.#outbox.set(id, text);
	}

	/** Pushes naming changed data; returns an unsubscribe function. */
	onInvalidate(listener: (keys: Invalidation[]) => void): () => void {
		this.#invalidateListeners.add(listener);
		return () => this.#invalidateListeners.delete(listener);
	}

	/** The server dropped its invalidation state: refetch everything. */
	onReset(listener: () => void): () => void {
		this.#resetListeners.add(listener);
		return () => this.#resetListeners.delete(listener);
	}

	/** Every `hello` after the first, i.e. a successful reconnect. */
	onReconnected(listener: () => void): () => void {
		this.#connectedListeners.add(listener);
		return () => this.#connectedListeners.delete(listener);
	}

	#receive(frame: Frame): void {
		switch (frame.type) {
			case 'hello': {
				if (frame.protocol !== PROTOCOL_VERSION) {
					this.fatal =
						frame.protocol > PROTOCOL_VERSION
							? 'This page is out of date. Reload to update it.'
							: 'Cohesion is older than this page. Update Cohesion, then reload.';
					this.close();
					return;
				}
				vocabulary.value = frame.vocabulary;
				this.status = 'connected';
				this.#greeted = true;
				this.#backoff = MIN_BACKOFF;
				// Its `accepted` reply matches nothing pending, so it's dropped.
				this.#socket?.send(JSON.stringify({ type: 'subscribe', id: this.#nextId++ }));
				for (const text of this.#outbox.values()) this.#socket?.send(text);
				this.#outbox.clear();
				const reconnected = this.#everGreeted;
				this.#everGreeted = true;
				if (reconnected) for (const listener of this.#connectedListeners) listener();
				return;
			}
			case 'reply': {
				if (frame.id === null) return;
				const pending = this.#pending.get(frame.id);
				if (!pending) return;
				this.#pending.delete(frame.id);
				pending.settle('error' in frame ? { error: frame.error } : { ok: frame.ok });
				return;
			}
			case 'invalidate':
				for (const listener of this.#invalidateListeners) listener(frame.keys);
				return;
			case 'reset':
				for (const listener of this.#resetListeners) listener();
				return;
			case 'evicted':
				this.fatal = 'This client was revoked. Pair it again to reconnect.';
				this.close();
				return;
		}
	}

	#handleClose(socket: WebSocket): void {
		if (socket !== this.#socket) return;
		this.#socket = null;
		const refused = !this.#greeted && !this.#everGreeted;
		this.#greeted = false;
		const error = new ProtocolError('disconnected', 'Lost the connection to Cohesion.');
		// Unsent requests never reached the daemon, so they wait for the next
		// socket, unless there won't be one.
		if (this.#closed) this.#outbox.clear();
		for (const [id, pending] of this.#pending) {
			if (this.#outbox.has(id)) continue;
			pending.reject(error);
			this.#pending.delete(id);
		}
		if (this.#closed) return;
		if (refused) this.#options.onRefused?.();
		this.status = 'reconnecting';
		this.#timer = setTimeout(() => {
			this.#timer = null;
			this.connect();
		}, this.#backoff);
		this.#backoff = Math.min(this.#backoff * 2, MAX_BACKOFF);
	}
}
