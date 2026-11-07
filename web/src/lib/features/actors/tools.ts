// How a tool an actor ran is shown in a session's transcript: an icon for its
// category, which the actor's client picked, and whether it (or the
// actor's thinking) is still going on.

import BookOpen from '@lucide/svelte/icons/book-open';
import Bot from '@lucide/svelte/icons/bot';
import FileText from '@lucide/svelte/icons/file-text';
import Globe from '@lucide/svelte/icons/globe';
import Pencil from '@lucide/svelte/icons/pencil';
import Search from '@lucide/svelte/icons/search';
import Terminal from '@lucide/svelte/icons/terminal';
import Wrench from '@lucide/svelte/icons/wrench';
import type {
	ActorSessionMessageContent,
	ActorSessionStatus,
	ActorToolKind
} from '$lib/protocol/types';

export type ToolIcon = typeof Wrench;

/** Each category's icon. */
export const TOOL_ICONS: Record<ActorToolKind, ToolIcon> = {
	command: Terminal,
	read: FileText,
	edit: Pencil,
	search: Search,
	web: Globe,
	agent: Bot,
	skill: BookOpen,
	other: Wrench
};

/** The icon for a tool of `category`; one left out is `other`'s. */
export function toolIcon(category: ActorToolKind | undefined): ToolIcon {
	return TOOL_ICONS[category ?? 'other'];
}

/**
 * Whether `said` is a tool or thinking still going on: its session works and
 * its actor hasn't said it ended. One whose actor never says (an older
 * client) stops once the session stops working.
 */
export function stillRunning(
	status: ActorSessionStatus | undefined,
	said: ActorSessionMessageContent
): boolean {
	return (
		status === 'working' && said.sender === 'actor' && 'ended_at' in said && said.ended_at === null
	);
}
