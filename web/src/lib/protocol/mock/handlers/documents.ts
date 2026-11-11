// Documents, journals and their folders.

import { ProtocolError } from '../../errors';
import type { Document, DocumentFolder } from '../../types';
import { nowSeconds } from '../support';
import { computeFormulas } from '../tableRules';
import type { Handlers, MockCore } from '../core';

export function documentsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		workspace,
		document,
		liveDocumentFolders,
		documentFolder,
		documentSubtree,
		listedFolder,
		tablesChanged,
		journalFor,
		keepDocument,
		touch,
		requireTitle
	} = core;
	return {
		getJournal: ({ workspace_id, date }) => {
			workspace(workspace_id);
			const existing = state.documents.find(
				(d) => d.workspace_id === workspace_id && d.journal_date === date && !d.deleted
			);
			return existing ? { ...existing } : null;
		},
		editJournal: ({ workspace_id, date, content }) =>
			touch(journalFor(workspace_id, date), content),
		appendToJournal: ({ workspace_id, date, content }) => {
			const d = journalFor(workspace_id, date);
			const joined = d.contents === '' ? content : `${d.contents.replace(/\n*$/, '')}\n${content}`;
			return touch(d, joined);
		},
		searchDocuments: ({ workspace_id, query }) => {
			const q = query.toLowerCase();
			return state.documents
				.filter(
					(d) => d.workspace_id === workspace_id && !d.deleted && d.title.toLowerCase().includes(q)
				)
				.map((d) => ({ ...d }));
		},
		listDocuments: ({ workspace_id, document_type, journal_from, journal_to }) =>
			state.documents
				.filter(
					(d) =>
						d.workspace_id === workspace_id &&
						!d.deleted &&
						(document_type == null || d.document_type === document_type) &&
						(journal_from == null || (d.journal_date !== null && d.journal_date >= journal_from)) &&
						(journal_to == null || (d.journal_date !== null && d.journal_date < journal_to))
				)
				.map((d) => ({ ...d })),
		getDocument: ({ workspace_id, document_id }) => ({ ...document(workspace_id, document_id) }),
		editDocument: ({ workspace_id, document_id, contents }) =>
			touch(document(workspace_id, document_id), contents),
		createDocument: ({ workspace_id, title, contents, folder_id = null }) => {
			workspace(workspace_id);
			requireTitle(title);
			if (folder_id !== null) documentFolder(workspace_id, folder_id);
			const created: Document = {
				id: nextId(state.documents),
				workspace_id,
				document_type: 'normal',
				title,
				contents,
				modified: nowSeconds(),
				journal_date: null,
				deleted: false,
				folder_id
			};
			state.documents.push(created);
			keepDocument(created, 'created');
			if (folder_id !== null) {
				computeFormulas(state, folder_id);
				tablesChanged(folder_id);
			}
			return { ...created };
		},
		moveDocument: ({ workspace_id, document_id, folder_id = null }) => {
			const d = document(workspace_id, document_id);
			if (d.document_type === 'journal')
				throw new ProtocolError('validationFailed', 'A journal can’t go in a folder.');
			if (folder_id !== null) documentFolder(workspace_id, folder_id);
			const from = d.folder_id;
			if (from === folder_id) return { ...d };
			// Leaving a folder lets go of its values; arriving in a table brings none.
			state.documentValues = state.documentValues.filter((v) => v.document_id !== d.id);
			d.folder_id = folder_id;
			if (folder_id !== null) computeFormulas(state, folder_id);
			tablesChanged(from, folder_id);
			return { ...d };
		},
		listDocumentFolders: ({ workspace_id }) => {
			workspace(workspace_id);
			return liveDocumentFolders(workspace_id).map(listedFolder);
		},
		createDocumentFolder: ({ workspace_id, title, parent_id = null }) => {
			workspace(workspace_id);
			requireTitle(title);
			if (parent_id !== null) documentFolder(workspace_id, parent_id);
			const created: DocumentFolder = {
				id: nextId(state.documentFolders),
				workspace_id,
				parent_id,
				title,
				sort_order:
					Math.max(
						0,
						...state.documentFolders
							.filter((f) => f.workspace_id === workspace_id)
							.map((f) => f.sort_order)
					) + 1,
				deleted: false,
				is_table: false
			};
			state.documentFolders.push(created);
			return { ...created };
		},
		renameDocumentFolder: ({ workspace_id, folder_id, title }) => {
			requireTitle(title);
			const f = documentFolder(workspace_id, folder_id);
			f.title = title;
			return listedFolder(f);
		},
		moveDocumentFolder: ({ workspace_id, folder_id, parent_id = null, before_id = null }) => {
			const f = documentFolder(workspace_id, folder_id);
			if (parent_id !== null) {
				documentFolder(workspace_id, parent_id);
				if (documentSubtree(workspace_id, folder_id).includes(parent_id))
					throw new ProtocolError(
						'validationFailed',
						'A folder can’t move into itself or a folder inside it.'
					);
			}
			const siblings = liveDocumentFolders(workspace_id).filter(
				(s) => s.parent_id === parent_id && s.id !== folder_id
			);
			const at =
				before_id === null ? siblings.length : siblings.findIndex((s) => s.id === before_id);
			if (at < 0)
				throw new ProtocolError(
					'validationFailed',
					'The folder to go before isn’t in that folder.'
				);
			siblings.splice(at, 0, f);
			f.parent_id = parent_id;
			siblings.forEach((s, i) => (s.sort_order = i));
			return liveDocumentFolders(workspace_id).map(listedFolder);
		},
		deleteDocumentFolder: ({ workspace_id, folder_id }) => {
			const f = documentFolder(workspace_id, folder_id);
			const doomed = documentSubtree(workspace_id, folder_id);
			const deleted = listedFolder(f);
			// The daemon moves their documents, deleted ones too, to the Inbox, and a
			// table among them loses its properties and its documents their values.
			for (const d of state.documents)
				if (
					d.workspace_id === workspace_id &&
					d.folder_id !== null &&
					doomed.includes(d.folder_id)
				) {
					d.folder_id = null;
					state.documentValues = state.documentValues.filter((v) => v.document_id !== d.id);
				}
			state.folderProperties = state.folderProperties.filter((p) => !doomed.includes(p.folder_id));
			for (const id of doomed) documentFolder(workspace_id, id).deleted = true;
			tablesChanged(...doomed);
			return deleted;
		},
		renameDocument: ({ workspace_id, document_id, title }) => {
			requireTitle(title);
			const d = document(workspace_id, document_id);
			d.title = title;
			keepDocument(d, 'edited');
			tablesChanged(d.folder_id);
			return { ...d };
		},
		deleteDocument: ({ workspace_id, document_id }) => {
			const d = document(workspace_id, document_id);
			d.deleted = true;
			keepDocument(d, 'deleted');
			tablesChanged(d.folder_id);
			return { ...d };
		}
	} satisfies Partial<Handlers>;
}
