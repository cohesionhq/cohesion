import { send } from '$lib/protocol/api';
import type {
	Actor,
	ActorSession,
	ActorSessionMessage,
	Contact,
	Document,
	IntelligenceAgent,
	Tag,
	Task,
	TaskFolder
} from '$lib/protocol/types';

import { toasts } from '$lib/ui/toasts.svelte';
import { findDocument, findTask, found } from './links';
import type { ItemLink } from './routes';
import { mainStores } from '$lib/stores/pageStores.svelte';

const { agents } = mainStores;

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
	| { kind: 'agent'; agent: IntelligenceAgent }
	| {
			kind: 'actor-session';
			session: ActorSession;
			/** Its actor, unless no client offers it any longer. */
			actor: Actor | null;
			messages: ActorSessionMessage[];
	  };

/**
 * The item a `cohesion://` link or a forward opens: one panel, mounted once
 * over the page beside the toasts, so the page stays where it is.
 */
class LinkedItems {
	item = $state<LinkedItem | null>(null);
	open = $state(false);

	/** Finds the item `link` names and opens it, or says it is gone. */
	async show(link: ItemLink): Promise<void> {
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
	 * page goes to it.
	 */
	follow(event: MouseEvent, link: ItemLink): void {
		if (event.button !== 0) return;
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		event.preventDefault();
		void this.show(link);
	}
}

const NAMES: Record<ItemLink['kind'], string> = {
	task: 'Task',
	document: 'Document',
	agent: 'Agent',
	'actor-session': 'Actor session'
};

async function find(link: ItemLink): Promise<LinkedItem | null> {
	switch (link.kind) {
		case 'task': {
			const task = await findTask(link.ws, link.id);
			if (task === null) return null;
			const [folders, tags, contacts] = await Promise.all([
				send('listTaskFolders', { workspace_id: task.workspace_id }),
				send('listTags', { workspace_id: task.workspace_id }),
				send('listContacts', { workspace_id: task.workspace_id })
			]);
			return { kind: 'task', task, folders, tags, contacts };
		}
		case 'document': {
			const document = await findDocument(link.ws, link.id);
			return document && { kind: 'document', document };
		}
		case 'agent': {
			await agents.loadList();
			const agent = agents.list.value.find((a) => a.id === link.id);
			return agent ? { kind: 'agent', agent } : null;
		}
		case 'actor-session': {
			const session = await found(send('getActorSession', { id: link.id }));
			if (session === null) return null;
			const [actors, messages] = await Promise.all([
				send('listActors'),
				send('listActorSessionMessages', { session_id: session.id })
			]);
			const actor = actors.find((a) => a.id === session.actor_id) ?? null;
			return { kind: 'actor-session', session, actor, messages };
		}
	}
}

export const linkedItem = new LinkedItems();
