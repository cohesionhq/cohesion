import { localDay } from '$lib/format/dates';
import { send as defaultSend } from '$lib/protocol/api';
import { ProtocolError } from '$lib/protocol/errors';
import type { Document, Send } from '$lib/protocol/types';
import { Draft } from './draft.svelte';
import { Resource } from './resource.svelte';

/** Today's journal for one workspace, with a local draft that autosaves. */
export class JournalStore {
	readonly today: Resource<Document | null, number>;
	readonly text: Draft;
	/** The day that was loaded; saves go here even after midnight passes. */
	#date = localDay();

	constructor(send: Send) {
		this.text = new Draft(async (content) => {
			// The entry is created on its first save if nothing was written that day.
			const doc = await send('editJournal', {
				workspace_id: this.today.key!,
				date: this.#date,
				content
			});
			this.today.set(doc);
			return doc.modified;
		});
		this.today = new Resource({
			fetch: (workspace_id) => {
				this.#date = localDay();
				return send('getJournal', { workspace_id, date: this.#date });
			},
			initial: null,
			matches: (key, workspace_id) => key.kind === 'documents' && key.id === workspace_id,
			canRefetch: () => !this.text.dirty && !this.text.saving,
			onFetched: (doc) => {
				if (doc) this.text.receive(doc.contents, doc.modified);
				else this.text.reset();
			}
		});
	}

	/** Takes what restoring a version of today's entry left, over any draft of it. */
	restored(doc: Document): void {
		if (this.today.value?.id !== doc.id) return;
		this.today.set(doc);
		this.text.reset(doc.contents, doc.modified);
	}

	/** What the editor shows; may be ahead of the server. */
	get draft(): string {
		return this.text.draft;
	}
	get saving(): boolean {
		return this.text.saving;
	}
	get saveError(): ProtocolError | null {
		return this.text.saveError;
	}
	/** Unix seconds of the last confirmed save (or load). */
	get savedAt(): number | null {
		return this.text.savedAt;
	}
	get dirty(): boolean {
		return this.text.dirty;
	}

	/** Saves anything pending for the previous workspace, then loads today's journal. */
	async open(workspace_id: number): Promise<void> {
		if (this.today.key !== workspace_id) {
			await this.flush().catch(() => undefined);
			this.text.reset();
		}
		await this.today.load(workspace_id);
	}

	/**
	 * Adds `text` to the end of today's entry as a paragraph of its own, then
	 * saves, so a note can be taken without opening the entry. The entry is
	 * loaded first, so what is already written is never replaced: when it
	 * can't be loaded, nothing is saved and the load's error is thrown.
	 */
	async append(workspace_id: number, text: string): Promise<void> {
		await this.open(workspace_id);
		// A failed load leaves the draft empty, and saving the note onto that
		// would replace the whole entry with it.
		if (!this.today.loaded) {
			throw (
				this.today.error ??
				new ProtocolError('internalError', 'Couldn’t load today’s journal. Try again.')
			);
		}
		const written = this.text.draft.trimEnd();
		this.text.edit(written === '' ? text.trim() : `${written}\n\n${text.trim()}`);
		await this.flush();
	}

	/** Records an edit and schedules an autosave. */
	edit(text: string): void {
		this.text.edit(text);
	}

	/** Saves the draft now if it differs from the server's copy. */
	flush(): Promise<void> {
		return this.text.flush();
	}
}

export function createJournalStore(send: Send = defaultSend): JournalStore {
	return new JournalStore(send);
}
