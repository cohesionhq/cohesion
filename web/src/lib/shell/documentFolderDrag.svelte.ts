// Dragging in the sidebar's document folders: a folder onto another to nest it, or onto a
// folder's top or bottom edge to put it beside that one; a document (or a table's row) onto a
// folder or the Inbox to file it there. Alt+↑/↓ moves a folder among its siblings, as task
// folders reorder. (Alt+←/→ are the app's back and forward.)

import { DOCUMENT_DRAG, FOLDER_DRAG } from '$lib/features/documents/drag';
import { siblings, subtreeIds } from '$lib/features/documents/folderTree';
import type { DocumentFolder } from '$lib/protocol/types';

type Where = 'before' | 'after' | 'into';

const carries = (event: DragEvent, type: string) =>
	event.dataTransfer?.types.includes(type) ?? false;

export class DocumentFolderDrag {
	/** The folder being dragged. */
	dragged = $state<DocumentFolder | null>(null);
	/** Where a drop would land: a folder's id or the Inbox, and how. */
	drop = $state<{ target: number | 'inbox'; where: Where } | null>(null);

	readonly #folders: () => DocumentFolder[];
	readonly #onmove: (
		folder: DocumentFolder,
		parent_id: number | null,
		before_id: number | null
	) => void;
	readonly #onfile: (document_id: number, folder_id: number | null) => void;

	constructor(
		folders: () => DocumentFolder[],
		/** Moves a folder into `parent_id` (the top level when null), before `before_id` or last. */
		onmove: (folder: DocumentFolder, parent_id: number | null, before_id: number | null) => void,
		/** Files a dragged document in `folder_id`, or the Inbox when it's null. */
		onfile: (document_id: number, folder_id: number | null) => void
	) {
		this.#folders = folders;
		this.#onmove = onmove;
		this.#onfile = onfile;
	}

	/** Where a drag over `folder`'s row would land, or null where it can't. */
	#landing(event: DragEvent, folder: DocumentFolder): Where | null {
		if (carries(event, DOCUMENT_DRAG)) return 'into';
		if (!this.dragged || !carries(event, FOLDER_DRAG)) return null;
		if (subtreeIds(this.#folders(), this.dragged.id).includes(folder.id)) return null;
		const { top, height } = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const y = (event.clientY - top) / height;
		return y < 0.25 ? 'before' : y > 0.75 ? 'after' : 'into';
	}

	over(event: DragEvent, folder: DocumentFolder) {
		const where = this.#landing(event, folder);
		if (where === null) return;
		event.preventDefault();
		event.dataTransfer!.dropEffect = 'move';
		this.drop = { target: folder.id, where };
	}

	dropOn(event: DragEvent, folder: DocumentFolder) {
		const where = this.#landing(event, folder);
		this.drop = null;
		if (where === null) return;
		event.preventDefault();
		const documentId = event.dataTransfer!.getData(DOCUMENT_DRAG);
		if (documentId) return this.#onfile(Number(documentId), folder.id);
		const moving = this.dragged!;
		if (where === 'into') {
			if (moving.parent_id !== folder.id) this.#onmove(moving, folder.id, null);
			return;
		}
		const row = siblings(this.#folders(), folder).filter((f) => f.id !== moving.id);
		const after = row[row.findIndex((f) => f.id === folder.id) + 1];
		this.#onmove(moving, folder.parent_id, where === 'before' ? folder.id : (after?.id ?? null));
	}

	// The Inbox takes a document, and a folder there goes to the top level.
	overInbox(event: DragEvent) {
		if (!carries(event, DOCUMENT_DRAG) && !(this.dragged && carries(event, FOLDER_DRAG))) return;
		event.preventDefault();
		this.drop = { target: 'inbox', where: 'into' };
	}

	dropOnInbox(event: DragEvent) {
		this.drop = null;
		event.preventDefault();
		const documentId = event.dataTransfer?.getData(DOCUMENT_DRAG);
		if (documentId) this.#onfile(Number(documentId), null);
		else if (this.dragged && this.dragged.parent_id !== null)
			this.#onmove(this.dragged, null, null);
	}

	start(event: DragEvent, folder: DocumentFolder) {
		this.dragged = folder;
		event.dataTransfer!.effectAllowed = 'move';
		event.dataTransfer!.setData(FOLDER_DRAG, String(folder.id));
		// Firefox only starts a drag that carries data; a drop elsewhere gets the title.
		event.dataTransfer!.setData('text/plain', folder.title);
	}

	end() {
		this.dragged = null;
		this.drop = null;
	}

	leave(event: DragEvent) {
		const to = event.relatedTarget as Node | null;
		if (!to || !(event.currentTarget as HTMLElement).contains(to)) this.drop = null;
	}

	key(event: KeyboardEvent, folder: DocumentFolder) {
		if (!event.altKey) return;
		const row = siblings(this.#folders(), folder);
		const index = row.findIndex((f) => f.id === folder.id);
		if (event.key === 'ArrowUp' && index > 0)
			this.#onmove(folder, folder.parent_id, row[index - 1].id);
		else if (event.key === 'ArrowDown' && index < row.length - 1)
			this.#onmove(folder, folder.parent_id, row[index + 2]?.id ?? null);
		else return;
		event.preventDefault();
	}
}
