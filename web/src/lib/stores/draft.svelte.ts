// A local copy of one document's text that runs ahead of the server and
// saves itself: the editor writes to `draft`, a debounce (or `flush`) sends
// it, and fetched copies only land while there is nothing unsaved.

import { toProtocolError, type ProtocolError } from '$lib/protocol/errors';

const AUTOSAVE_MS = 800;

/** Sends `content` and resolves with the saved copy's `modified` time. */
export type SaveDraft = (content: string) => Promise<number>;

export class Draft {
	/** What the editor shows; may be ahead of the server. */
	draft = $state('');
	/** The contents the server last confirmed. */
	saved = $state('');
	saving = $state(false);
	saveError = $state<ProtocolError | null>(null);
	/** Unix seconds of the last confirmed save (or load). */
	savedAt = $state<number | null>(null);
	dirty = $derived(this.draft !== this.saved);

	#save: SaveDraft;
	#timer: ReturnType<typeof setTimeout> | null = null;
	#inflight: Promise<void> | null = null;

	constructor(save: SaveDraft) {
		this.#save = save;
	}

	/** Starts over from a server copy (or nothing), dropping any local edits. */
	reset(contents = '', savedAt: number | null = null): void {
		this.draft = contents;
		this.saved = contents;
		this.savedAt = savedAt;
		this.saveError = null;
	}

	/** A fetched copy; ignored while there are unsaved edits. */
	receive(contents: string, modified: number): void {
		if (this.dirty) return;
		this.reset(contents, modified);
	}

	/** Records an edit and schedules an autosave. */
	edit(text: string): void {
		this.draft = text;
		this.saveError = null;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			this.#timer = null;
			this.flush().catch(() => undefined);
		}, AUTOSAVE_MS);
	}

	/** Saves the draft now if it differs from the server's copy. */
	async flush(): Promise<void> {
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = null;
		if (this.#inflight) await this.#inflight;
		if (!this.dirty) return;
		const content = this.draft;
		this.saving = true;
		this.#inflight = this.#save(content)
			.then((modified) => {
				this.saved = content;
				this.savedAt = modified;
			})
			.catch((error: unknown) => {
				this.saveError = toProtocolError(error);
				throw this.saveError;
			})
			.finally(() => {
				this.saving = false;
				this.#inflight = null;
			});
		await this.#inflight;
	}

	/** "Saved · 09:05", "Saving…", "Not saved" or nothing, for a page header. */
	status(formatTime: (seconds: number) => string): string {
		if (this.saveError) return 'Not saved';
		if (this.saving || this.dirty) return 'Saving…';
		if (this.savedAt === null) return '';
		return `Saved · ${formatTime(this.savedAt)}`;
	}
}
