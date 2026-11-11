import { send as defaultSend } from '$lib/protocol/api';
import { monthLabel } from '$lib/format/dates';
import { subtreeIds } from '$lib/features/documents/folderTree';
import { holdsValues } from '$lib/features/tables/values';
import type { Document, DocumentFolder, Invalidation, Send } from '$lib/protocol/types';
import { Draft } from './draft.svelte';
import { Resource } from './resource.svelte';

export interface DocumentKey {
	workspace_id: number;
	document_id: number;
}

export interface Group<T> {
	key: string;
	label: string;
	items: T[];
}

/** Above this many documents the list filter asks the daemon instead of filtering locally. */
export const SERVER_FILTER_THRESHOLD = 200;

const byModified = (a: Document, b: Document) => b.modified - a.modified;
// Journals always have a date; "YYYY-MM-DD" days sort as text.
const byJournalDate = (a: Document, b: Document) => b.journal_date!.localeCompare(a.journal_date!);

/** Splits documents (newest first) into Today, This week (the last seven days) and Earlier. */
export function groupByRecency(docs: Document[], todayStart: number): Group<Document>[] {
	const weekStart = todayStart - 6 * 86_400;
	const groups: Group<Document>[] = [
		{ key: 'today', label: 'Today', items: [] },
		{ key: 'week', label: 'This week', items: [] },
		{ key: 'earlier', label: 'Earlier', items: [] }
	];
	for (const doc of [...docs].sort(byModified)) {
		const index = doc.modified >= todayStart ? 0 : doc.modified >= weekStart ? 1 : 2;
		groups[index].items.push(doc);
	}
	return groups.filter((g) => g.items.length > 0);
}

/** Groups journal entries by month, newest month and newest day first. */
export function groupByMonth(journals: Document[]): Group<Document>[] {
	const groups: Group<Document>[] = [];
	for (const doc of [...journals].sort(byJournalDate)) {
		const key = doc.journal_date!.slice(0, 7);
		const last = groups.at(-1);
		if (last?.key === key) last.items.push(doc);
		else groups.push({ key, label: monthLabel(key), items: [doc] });
	}
	return groups;
}

/**
 * The first non-empty line, without leading Markdown markers, code ticks or bold marks, cut to `max` characters;
 * a line that only repeats `skip` (a document's title as its first heading) is passed over.
 */
export function excerpt(contents: string, max = 90, skip?: string): string {
	const line =
		contents
			.split('\n')
			.map((l) =>
				l
					.replace(/^[\s#>*+-]+/, '')
					.replace(/`|\*\*/g, '')
					.trim()
			)
			.find((l) => l !== '' && l !== skip) ?? '';
	return line.length > max ? `${line.slice(0, max - 1).trimEnd()}…` : line;
}

/** The entries either side of `id` in the archive: `prev` is the older day, `next` the newer. */
export function neighbours(
	journals: Document[],
	id: number
): { prev: Document | null; next: Document | null } {
	const sorted = [...journals].sort(byJournalDate);
	const index = sorted.findIndex((d) => d.id === id);
	if (index < 0) return { prev: null, next: null };
	return { prev: sorted[index + 1] ?? null, next: sorted[index - 1] ?? null };
}

/** Normal documents and their folders, the journal archive, and the one open document with an autosaving draft. */
export class DocumentsStore {
	readonly list: Resource<Document[], number>;
	readonly folders: Resource<DocumentFolder[], number>;
	readonly archive: Resource<Document[], number>;
	readonly current: Resource<Document | null, DocumentKey>;
	readonly text: Draft;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		const inWorkspace = (key: Invalidation, workspace_id: number) =>
			key.kind === 'documents' && key.id === workspace_id;
		this.list = new Resource({
			fetch: (workspace_id) =>
				send('listDocuments', { workspace_id, document_type: 'normal' }).then((docs) =>
					docs.sort(byModified)
				),
			initial: [],
			matches: inWorkspace
		});
		this.folders = new Resource({
			fetch: (workspace_id) => send('listDocumentFolders', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'documentFolders' && key.id === workspace_id
		});
		this.archive = new Resource({
			fetch: (workspace_id) =>
				send('listDocuments', { workspace_id, document_type: 'journal' }).then((docs) =>
					docs.sort(byJournalDate)
				),
			initial: [],
			matches: inWorkspace
		});
		this.text = new Draft(async (contents) => {
			const { workspace_id, document_id } = this.current.key!;
			const doc = await send('editDocument', { workspace_id, document_id, contents });
			this.#apply(doc);
			return doc.modified;
		});
		this.current = new Resource({
			fetch: ({ workspace_id, document_id }) => send('getDocument', { workspace_id, document_id }),
			initial: null,
			matches: (key, { workspace_id }) => inWorkspace(key, workspace_id),
			canRefetch: () => !this.text.dirty && !this.text.saving,
			onFetched: (doc) => {
				if (doc) this.text.receive(doc.contents, doc.modified);
			}
		});
	}

	/** Saves anything pending for the previous document, then loads this one. */
	async open(workspace_id: number, document_id: number): Promise<void> {
		const current = this.current.key;
		if (current?.workspace_id === workspace_id && current.document_id === document_id) {
			await this.current.load(current);
			return;
		}
		await this.text.flush().catch(() => undefined);
		this.text.reset();
		await this.current.load({ workspace_id, document_id });
	}

	/** Title search on the daemon, for lists too long to filter locally. */
	search(workspace_id: number, query: string): Promise<Document[]> {
		return this.#send('searchDocuments', { workspace_id, query }).then((docs) =>
			docs.filter((d) => d.document_type === 'normal').sort(byModified)
		);
	}

	/** Creates a document in `folder_id`, or the Inbox when it's null. */
	async create(
		workspace_id: number,
		title = 'Untitled',
		contents = '',
		folder_id: number | null = null
	): Promise<Document> {
		const doc = await this.#send('createDocument', { workspace_id, title, contents, folder_id });
		if (this.list.key === workspace_id) this.list.update((list) => [doc, ...list]);
		return doc;
	}

	/**
	 * Files `doc` in `folder_id`, or the Inbox when it's null. Leaving a
	 * table loses the document's values (`moveLosesValues` says whether it
	 * holds any to lose); going into one gives it none.
	 */
	async move(
		doc: Pick<Document, 'workspace_id' | 'id'>,
		folder_id: number | null
	): Promise<Document> {
		const moved = await this.#send('moveDocument', {
			workspace_id: doc.workspace_id,
			document_id: doc.id,
			folder_id
		});
		this.#apply(moved);
		return moved;
	}

	/**
	 * Whether moving `doc` to `folder_id` (the Inbox when null) deletes values
	 * set by hand: it leaves a table, to the Inbox, a plain folder or another
	 * table, holding some. A formula's outputs alone aren't asked about. The
	 * one rule every way of moving a document asks before it moves one.
	 */
	async moveLosesValues(
		doc: Pick<Document, 'workspace_id' | 'id' | 'folder_id'>,
		folder_id: number | null
	): Promise<boolean> {
		const from = doc.folder_id;
		if (from === null || from === folder_id) return false;
		// A folder known not to be a table holds no values; one not listed here is asked.
		const listed = this.folders.key === doc.workspace_id ? this.folders.value : [];
		if (listed.find((f) => f.id === from)?.is_table === false) return false;
		const key = { workspace_id: doc.workspace_id, folder_id: from };
		const [properties, row] = await Promise.all([
			this.#send('listDocumentFolderProperties', key),
			this.#send('getTableRow', { ...key, document_id: doc.id })
		]);
		return holdsValues(row, properties);
	}

	/** Retitles document `document_id`, wherever it shows. */
	async renameDocument(
		workspace_id: number,
		document_id: number,
		title: string
	): Promise<Document> {
		const doc = await this.#send('renameDocument', { workspace_id, document_id, title });
		this.#apply(doc);
		return doc;
	}

	async createFolder(
		workspace_id: number,
		title: string,
		parent_id: number | null = null
	): Promise<DocumentFolder> {
		const folder = await this.#send('createDocumentFolder', { workspace_id, title, parent_id });
		if (this.folders.key === workspace_id) this.folders.update((list) => [...list, folder]);
		return folder;
	}

	async renameFolder(workspace_id: number, folder_id: number, title: string): Promise<void> {
		const folder = await this.#send('renameDocumentFolder', { workspace_id, folder_id, title });
		if (this.folders.key === workspace_id)
			this.folders.update((list) => list.map((f) => (f.id === folder.id ? folder : f)));
	}

	/** Moves a folder into `parent_id` (the top level when null), before `before_id` or last. */
	async moveFolder(
		workspace_id: number,
		folder_id: number,
		parent_id: number | null,
		before_id: number | null = null
	): Promise<void> {
		const folders = await this.#send('moveDocumentFolder', {
			workspace_id,
			folder_id,
			parent_id,
			before_id
		});
		if (this.folders.key === workspace_id) this.folders.set(folders);
	}

	/**
	 * Takes what a column's reply says of folder `folder_id`: its first made it
	 * a table, deleting its last a plain folder again.
	 */
	markTable(folder_id: number, is_table: boolean): void {
		this.folders.update((list) =>
			list.map((f) => (f.id === folder_id && f.is_table !== is_table ? { ...f, is_table } : f))
		);
	}

	/** Deletes a folder and those inside it; their documents move to the Inbox, as the daemon moves them. */
	async deleteFolder(workspace_id: number, folder_id: number): Promise<void> {
		await this.#send('deleteDocumentFolder', { workspace_id, folder_id });
		if (this.folders.key !== workspace_id) return;
		const gone = subtreeIds(this.folders.value, folder_id);
		this.folders.update((list) => list.filter((f) => !gone.includes(f.id)));
		const out = (d: Document) =>
			d.folder_id !== null && gone.includes(d.folder_id) ? { ...d, folder_id: null } : d;
		if (this.list.key === workspace_id) this.list.update((docs) => docs.map(out));
		const open = this.current.value;
		if (open && open.workspace_id === workspace_id) this.current.set(out(open));
	}

	/** Renames the open document. */
	rename(title: string): Promise<Document> {
		const { workspace_id, document_id } = this.current.key!;
		return this.renameDocument(workspace_id, document_id, title);
	}

	/** Takes what restoring a version of the open document left, over any draft of it. */
	restored(doc: Document): void {
		if (this.current.value?.id === doc.id) this.text.reset(doc.contents, doc.modified);
		this.#apply(doc);
	}

	/** Deletes document `document_id`, as a table's row is deleted, from wherever it shows. */
	async deleteDocument(workspace_id: number, document_id: number): Promise<Document> {
		if (this.current.key?.document_id === document_id) return this.remove();
		const doc = await this.#send('deleteDocument', { workspace_id, document_id });
		if (this.list.key === workspace_id)
			this.list.update((docs) => docs.filter((d) => d.id !== doc.id));
		return doc;
	}

	/** Deletes the open document, dropping any unsaved edits to it. */
	async remove(): Promise<Document> {
		const { workspace_id, document_id } = this.current.key!;
		const doc = await this.#send('deleteDocument', { workspace_id, document_id });
		this.text.reset();
		this.current.set(null);
		for (const list of [this.list, this.archive])
			if (list.key === workspace_id) list.update((docs) => docs.filter((d) => d.id !== doc.id));
		return doc;
	}

	/** Takes a reply for the open document into it and whichever list holds it. */
	#apply(doc: Document): void {
		if (this.current.value?.id === doc.id) this.current.set(doc);
		const list = doc.document_type === 'journal' ? this.archive : this.list;
		if (list.key === doc.workspace_id)
			list.update((docs) => docs.map((d) => (d.id === doc.id ? doc : d)));
	}
}

export function createDocumentsStore(send: Send = defaultSend): DocumentsStore {
	return new DocumentsStore(send);
}
