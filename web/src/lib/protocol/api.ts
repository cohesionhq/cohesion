// The app's single entry point to the daemon. `send` goes over the WebSocket
// (or the mock), which the daemon answers side by side, so a long request
// holds up nothing; `sendRaw` sends a request by name and keeps the whole
// reply frame. Nothing connects until `start()`, so importing this module is
// free in tests.

import { authHeaders, markUnauthorized, wsUrl } from './session';
import { Connection } from './connection.svelte';
import { createMock, MOCK_VOCABULARY } from './mock';
import type { RawRequest, ReplyFrame, Send } from './types';
import { vocabulary } from './vocabulary.svelte';
import { working } from './working.svelte';

export const mockMode = import.meta.env.VITE_MOCK === '1';

const mock = mockMode ? createMock() : null;

let unreachableListeners: (() => void)[] = [];

/** The socket was refused for a reason other than auth (e.g. a 502 from the proxy). */
export function onUnreachable(listener: () => void): () => void {
	unreachableListeners.push(listener);
	return () => {
		unreachableListeners = unreachableListeners.filter((l) => l !== listener);
	};
}

/**
 * A socket closed before `hello` can't say why, so ask over HTTP: a 401
 * means the token was refused; anything else is the daemon being away,
 * which the connection retries on its own.
 */
async function diagnoseRefusal(): Promise<void> {
	const status = await fetch('/client', { headers: authHeaders() }).then(
		(response) => response.status,
		() => 0
	);
	if (status === 401) markUnauthorized();
	else for (const listener of unreachableListeners) listener();
}

export const connection = new Connection({
	url: wsUrl,
	onRefused: () => void diagnoseRefusal()
});

/** Requests over the shared socket, counted by the working mark. */
export const send: Send = working.track(
	mock ? mock.send : (type, ...args) => connection.send(type, ...args)
);

/**
 * Any request by name, for the API console: resolves with the reply frame,
 * error replies included. Only transport failures throw.
 */
export const sendRaw: (request: RawRequest) => Promise<ReplyFrame> = mock
	? mock.sendRaw
	: (request) => connection.sendRaw(request);

let started = false;

export function start(): void {
	if (started) return;
	started = true;
	if (mock) {
		vocabulary.value = MOCK_VOCABULARY;
		connection.status = 'connected';
		mock.onInvalidate((keys) => connection.invalidated(keys));
		return;
	}
	connection.connect();
}
