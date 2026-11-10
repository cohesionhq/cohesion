import { send } from '$lib/protocol/api';
import type {
	Actor,
	ActorSession,
	ActorSessionMessage,
	Article,
	Contact,
	Document,
	IntelligenceAgent,
	Resource,
	Tag,
	Task,
	TaskFolder
} from '$lib/protocol/types';

import { goto } from '$app/navigation';
import { toasts } from '$lib/ui/toasts.svelte';
import { resolve } from './links';
import { itemLinkRoute, type ItemLink } from './routes';

/** What a link opened, with what its panel shows besides the item itself. */
export type LinkedItem =
	| {
			kind: 'task';
			task: Task;
			/** The folders, task tags and contacts of the task's workspace, which its panel picks from. */
			folders: TaskFolder[];
			tags: Tag[];
			contacts: Contact[];
	  }
	| { kind: 'document'; document: Document }
	| { kind: 'resource'; resource: Resource }
	| { kind: 'contact'; contact: Contact }
	| { kind: 'agent'; agent: IntelligenceAgent }
	| {
			kind: 'actor-session';
			session: ActorSession;
			/** Its actor, unless no client offers it any longer. */
			actor: Actor | null;
			messages: ActorSessionMessage[];
	  }
	| { kind: 'article'; article: Article; feedTitle: string }
	/** A document whose folder is a table, opened as its row, with its values. */
	| { kind: 'table-row'; document: Document };

/**
 * The item a `cohesion://` link or a forward opens: one panel, mounted once
 * over the page beside the toasts, so the page stays where it is.
 */
class LinkedItems {
	item = $state<LinkedItem | null>(null);
	open = $state(false);

	/**
	 * Finds the item `link` names and opens it, or says it is gone. A folder
	 * has no panel: it goes to the Documents page at it.
	 */
	async show(link: ItemLink): Promise<void> {
		if (link.kind === 'document-folder') {
			await goto(itemLinkRoute(link));
			return;
		}
		try {
			const item = await find(link);
			if (item === null) {
				toasts.show(`${NAMES[link.kind]} not found.`);
				return;
			}
			this.item = item;
			this.open = true;
		} catch (error) {
			toasts.error(error);
		}
	}

	/**
	 * A click on a link to `link` opens it over the page, unless it is
	 * opened elsewhere (a modifier or another button), when the link's Open
	 * page goes to it. A folder has no panel, so its link goes to the
	 * Documents page at it as any link would.
	 */
	follow(event: MouseEvent, link: ItemLink): void {
		if (event.button !== 0 || link.kind === 'document-folder') return;
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		event.preventDefault();
		void this.show(link);
	}
}

const NAMES: Record<ItemLink['kind'], string> = {
	task: 'Task',
	document: 'Document',
	resource: 'Resource',
	'document-folder': 'Folder',
	contact: 'Contact',
	agent: 'Agent',
	'actor-session': 'Actor session',
	article: 'Article'
};

/** What `link` names, as the daemon resolves it, with what its panel needs besides. */
async function find(link: ItemLink): Promise<LinkedItem | null> {
	const embed = await resolve(link);
	if (embed === null) return null;
	const { item } = embed;
	switch (item.kind) {
		case 'task': {
			const { task } = item;
			const [folders, tags, contacts] = await Promise.all([
				send('listTaskFolders', { workspace_id: task.workspace_id }),
				send('listTags', { workspace_id: task.workspace_id }),
				send('listContacts', { workspace_id: task.workspace_id })
			]);
			return { kind: 'task', task, folders, tags, contacts };
		}
		case 'document': {
			const { document } = item;
			// A document in a table opens as its row, with its values.
			if (document.folder_id === null) return { kind: 'document', document };
			const properties = await send('listDocumentFolderProperties', {
				workspace_id: document.workspace_id,
				folder_id: document.folder_id
			});
			return { kind: properties.length > 0 ? 'table-row' : 'document', document };
		}
		case 'resource':
			return { kind: 'resource', resource: item.resource };
		case 'contact':
			return { kind: 'contact', contact: item.contact };
		case 'agent':
			return { kind: 'agent', agent: item.agent };
		case 'actorSession': {
			const { session } = item;
			const [actors, messages] = await Promise.all([
				send('listActors'),
				send('listActorSessionMessages', { session_id: session.id })
			]);
			const actor = actors.find((a) => a.id === session.actor_id) ?? null;
			return { kind: 'actor-session', session, actor, messages };
		}
		case 'article': {
			const article = await send('getArticle', { article_id: item.article.id });
			return { kind: 'article', article, feedTitle: item.feed_title };
		}
		// A folder has no panel; `show` sends it to the Documents page at it.
		case 'documentFolder':
			return null;
	}
}

export const linkedItem = new LinkedItems();
