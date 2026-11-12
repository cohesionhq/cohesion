// What the sidebar and the Documents and Tasks pages ask of the folder dialogs: a folder to
// create, rename or delete, or a saved task view to rename or delete. `FolderDialogs` (in the root layout) answers, so any page can.

import type { DocumentFolder, SavedTaskView, TaskFolder } from '$lib/protocol/types';

export type FolderRequest =
	| { kind: 'documents'; mode: 'new'; parent: DocumentFolder | null }
	| { kind: 'documents'; mode: 'rename'; folder: DocumentFolder }
	| { kind: 'documents'; mode: 'delete'; folder: DocumentFolder }
	| { kind: 'tasks'; mode: 'new' }
	| { kind: 'tasks'; mode: 'rename'; folder: TaskFolder }
	| { kind: 'tasks'; mode: 'delete'; folder: TaskFolder }
	| { kind: 'views'; mode: 'rename'; view: SavedTaskView }
	| { kind: 'views'; mode: 'delete'; view: SavedTaskView };

class FolderDialogRequests {
	/** The latest request, until the dialogs take it. */
	pending = $state<FolderRequest | null>(null);

	request(next: FolderRequest) {
		this.pending = next;
	}
}

export const folderDialogs = new FolderDialogRequests();
