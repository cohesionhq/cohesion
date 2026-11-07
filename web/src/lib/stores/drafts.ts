// What the user has typed but not sent, kept in this browser so leaving a
// conversation or reloading the page doesn't lose it: a composer's message,
// or the answers picked so far on an actor's questions. Each draft lives
// under its own key, and an empty one (or one sent) is forgotten.

const PREFIX = 'cohesion.draft.';

/** The draft kept under `key`, or `fallback` when there is none (or storage is blocked). */
export function rememberedDraft<T>(key: string, fallback: T): T {
	try {
		const stored = localStorage.getItem(PREFIX + key);
		return stored === null ? fallback : (JSON.parse(stored) as T);
	} catch {
		return fallback;
	}
}

/** Keeps `value` under `key`; `null` forgets it. */
export function rememberDraft(key: string, value: unknown): void {
	try {
		if (value === null) localStorage.removeItem(PREFIX + key);
		else localStorage.setItem(PREFIX + key, JSON.stringify(value));
	} catch {
		// Keeping drafts is a convenience; ignore blocked or full storage.
	}
}

/** A composer's text kept under `key`; '' when there is none. */
export function rememberedText(key: string): string {
	const stored = rememberedDraft<unknown>(key, '');
	return typeof stored === 'string' ? stored : '';
}

/** Keeps a composer's text under `key`, forgetting it once empty. */
export function rememberText(key: string, text: string): void {
	rememberDraft(key, text.trim() === '' ? null : text);
}
