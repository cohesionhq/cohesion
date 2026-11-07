import { longDay } from '$lib/format/dates';
import type {
	AgentForwardTarget,
	AgentForwardedItem,
	AgentForwardedItemKind,
	IntelligenceAgent
} from '$lib/protocol/types';
import type { ItemLink } from '$lib/shell/routes';
import type { MenuSubmenu } from '$lib/ui/Menu.svelte';
import { mainStores } from '$lib/stores/pageStores.svelte';

const { agents } = mainStores;

/**
 * An item forwarded to an agent, which reads it from its link with its code
 * tool; a journal entry's title is its day, `YYYY-MM-DD`.
 */
export type ForwardedItem = AgentForwardedItem;

// `link` is the kind a `cohesion://` link names it by: a journal entry is a document.
const kinds: Record<AgentForwardedItemKind, { label: string; link: 'task' | 'document' }> = {
	task: { label: 'Task', link: 'task' },
	document: { label: 'Document', link: 'document' },
	journal: { label: 'Journal entry', link: 'document' }
};

/** A message being written to an agent about an item elsewhere in the app. */
export interface ForwardDraft {
	agent: IntelligenceAgent;
	/** The item to forward, or null once taken off: the note then goes alone. */
	item: ForwardedItem | null;
}

/**
 * What "Forward" opens: one composer, mounted once over the page beside the
 * toasts, that any menu fills with an item and the agent to send it to. The
 * page stays where it is.
 */
class Forwarding {
	draft = $state<ForwardDraft | null>(null);

	/**
	 * A "Forward" submenu listing every agent; choosing one opens the
	 * composer on `item()`, read as the agent is chosen.
	 */
	submenu(item: () => ForwardedItem): MenuSubmenu {
		const list = agents.list.value;
		return {
			label: 'Forward',
			disabled: list.length === 0,
			detail: list.length === 0 ? 'No agents yet.' : undefined,
			items: list.map((agent) => ({
				id: agent.id,
				label: agent.agent_name,
				avatar: { name: agent.agent_name, color: agent.agent_color, emoji: agent.agent_emoji },
				onSelect: () => (this.draft = { agent, item: item() })
			}))
		};
	}
}

export const forwarding = new Forwarding();

/** The item's title as the app shows it: a journal entry's day in words. */
export function forwardedTitle(item: ForwardedItem): string {
	return item.kind === 'journal' ? longDay(item.title) : item.title;
}

/** The item as the app shows it: `Task “Fix login” #42`. */
export function forwardedLabel(item: ForwardedItem): string {
	const head = `${kinds[item.kind].label} “${forwardedTitle(item)}”`;
	// A task's id is the one the app shows (Copy ID, `#42` in lists).
	return item.kind === 'task' ? `${head} #${item.id}` : head;
}

/** What kind of item it is, `Task #42` or `Document`: the chip's quiet half beside the title. */
export function forwardedKind(item: ForwardedItem): string {
	const label = kinds[item.kind].label;
	return item.kind === 'task' ? `${label} #${item.id}` : label;
}

/** The `cohesion://` link the item opens as. */
export function forwardedLink(item: ForwardedItem): ItemLink {
	return { kind: kinds[item.kind].link, ws: item.workspace_id, id: item.id };
}

/** What a message forwarding the item names it by; the daemon fills in the rest. */
export function forwardTarget(item: ForwardedItem): AgentForwardTarget {
	return { kind: kinds[item.kind].link, workspace_id: item.workspace_id, id: item.id };
}
