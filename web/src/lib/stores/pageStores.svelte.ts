// The stores that hold what a page shows (an open task list, document,
// table, conversation or actor session, a workspace's tags or contacts), one set for each half of
// the sheet, so two pages of the same kind side by side each have their own.
// A component takes its half's with `pageStores()`; the shell (sidebar, ⌘K,
// the linked-item panel), drawn outside both halves, gets the main half's.
// Lists every half shares (workspaces, feeds, connectors, extensions) stay
// app-wide singletons in their own modules.

import { getContext, setContext } from 'svelte';
import { send as defaultSend } from '$lib/protocol/api';
import type { Send } from '$lib/protocol/types';
import { createActorsStore, type ActorsStore } from './actors.svelte';
import { createAgentsStore, type AgentsStore } from './agents.svelte';
import { createArticlesStore, type ArticlesStore } from './articles.svelte';
import { createContactsStore, type ContactsStore } from './contacts.svelte';

import { createDocumentsStore, type DocumentsStore } from './documents.svelte';
import { createJournalStore, type JournalStore } from './journal.svelte';
import { Resource } from './resource.svelte';
import { createResourcesStore, type ResourcesStore } from './resources.svelte';
import { createRewindStore, type RewindStore } from './rewind.svelte';
import { createTablesStore, DocumentValues, type TablesStore } from './tables.svelte';
import { createTagsStore, type TagsStore } from './tags.svelte';
import { createTasksStore, type TasksStore } from './tasks.svelte';
import { createThreadsStore, type ThreadsStore } from './threads.svelte';

export interface PageStores {
	actors: ActorsStore;
	agents: AgentsStore;
	articles: ArticlesStore;
	contacts: ContactsStore;
	documents: DocumentsStore;
	journal: JournalStore;
	resources: ResourcesStore;
	rewind: RewindStore;
	tables: TablesStore;
	tasks: TasksStore;
	threads: ThreadsStore;
	taskTags: TagsStore;
}

export function createPageStores(send: Send = defaultSend): PageStores {
	return {
		actors: createActorsStore(send),
		agents: createAgentsStore(send),
		articles: createArticlesStore(send),
		contacts: createContactsStore(send),
		documents: createDocumentsStore(send),
		journal: createJournalStore(send),
		resources: createResourcesStore(send),
		rewind: createRewindStore(send),
		tables: createTablesStore(send),
		tasks: createTasksStore(send),
		threads: createThreadsStore(send),
		taskTags: createTagsStore(send)
	};
}

/** The main half's, which the shell shows too. */
export const mainStores = createPageStores();

const KEY = Symbol('page-stores');

/** Makes `stores` the ones the components below use. */
export function providePageStores(stores: PageStores) {
	setContext(KEY, stores);
}

/** The stores of the half this component is drawn in; outside both, the main half's. */
export function pageStores(): PageStores {
	return getContext<PageStores | undefined>(KEY) ?? mainStores;
}

/** Stops every resource in `stores` refetching, once the half that showed them has closed. */
export function dropPageStores(stores: PageStores) {
	const drop = (holder: object) => {
		for (const value of Object.values(holder)) {
			if (value instanceof Resource) value.drop();
			// A part of a store with resources of its own.
			else if (value instanceof DocumentValues) drop(value);
		}
	};
	for (const store of Object.values(stores)) drop(store);
}
