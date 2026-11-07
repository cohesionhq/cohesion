// The token `cohesion open` puts in the URL fragment, saved by the CLI so it
// stays the same across launches. Read once, stripped from the address bar,
// and kept in `localStorage` so every tab on this origin shares it, and a
// bookmark of the bare address works after a restart.

const STORAGE_KEY = 'cohesion.token';

let token: string | null = null;
let unauthorizedListeners: (() => void)[] = [];

/** Reads `#token=<t>` (or the stored token) and removes the fragment from the URL. */
export function initSession(): void {
	const match = /(?:^|[#&])token=([^&]+)/.exec(location.hash);
	if (match) {
		token = decodeURIComponent(match[1]);
		try {
			localStorage.setItem(STORAGE_KEY, token);
		} catch {
			// Storage can be unavailable; the in-memory token still works for this load.
		}
		history.replaceState(history.state, '', location.pathname + location.search);
		return;
	}
	try {
		token = localStorage.getItem(STORAGE_KEY);
	} catch {
		token = null;
	}
}

/** True when there is no way to authenticate: no token and not behind the dev proxy. */
export function missingToken(): boolean {
	return token === null && !import.meta.env.VITE_DEV_PROXY;
}

export function authHeaders(): Record<string, string> {
	return token ? { Authorization: `Bearer ${token}` } : {};
}

/** Absolute WebSocket URL for `/connect` on this origin. */
export function wsUrl(): string {
	const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
	const query = token ? `?token=${encodeURIComponent(token)}` : '';
	return `${scheme}://${location.host}/connect${query}`;
}

/** Reports that the proxy refused this tab's token. */
export function markUnauthorized(): void {
	for (const listener of unauthorizedListeners) listener();
}

export function onUnauthorized(listener: () => void): () => void {
	unauthorizedListeners.push(listener);
	return () => {
		unauthorizedListeners = unauthorizedListeners.filter((l) => l !== listener);
	};
}
