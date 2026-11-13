// How a permission is shown to a person: an icon beside the title and
// one-line description the daemon's vocabulary words it with. A hook is
// shown by its words alone. The review before installing and an
// extension's own page both draw grants this way.

import AlarmClock from '@lucide/svelte/icons/alarm-clock';
import Bell from '@lucide/svelte/icons/bell';
import Blocks from '@lucide/svelte/icons/blocks';
import Bot from '@lucide/svelte/icons/bot';
import CalendarSync from '@lucide/svelte/icons/calendar-sync';
import Cable from '@lucide/svelte/icons/cable';
import Eye from '@lucide/svelte/icons/eye';
import Gauge from '@lucide/svelte/icons/gauge';
import Globe from '@lucide/svelte/icons/globe';
import KeyRound from '@lucide/svelte/icons/key-round';
import MessageSquareLock from '@lucide/svelte/icons/message-square-lock';
import MessageSquareReply from '@lucide/svelte/icons/message-square-reply';
import PencilLine from '@lucide/svelte/icons/pencil-line';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import Store from '@lucide/svelte/icons/store';
import type { ExtensionPermissionKind } from '$lib/protocol/types';

export type GrantIcon = typeof Eye;

/** Each permission's icon. */
export const PERMISSION_ICONS: Record<ExtensionPermissionKind, GrantIcon> = {
	'cohesion:read': Eye,
	'cohesion:write': PencilLine,
	network: Globe,
	timer: AlarmClock,
	registry: Store,
	oauth: KeyRound,
	connector: Cable,
	'calendar:backend': CalendarSync,
	service: MessageSquareReply,
	'service:isolated': MessageSquareLock,
	services: Blocks,
	usage: Gauge,
	intelligence: Bot,
	notifications: Bell,
	actors: SquareTerminal
};

/**
 * What a person turns on before an extension runs, from what it asks for:
 * "permissions and hooks", or just one of them. A fresh install has all of
 * them off, and the copy around that names exactly these.
 */
export function grantNoun(permissions: number, hooks: number): string {
	if (permissions > 0 && hooks > 0) return 'permissions and hooks';
	return permissions > 0 ? 'permissions' : 'hooks';
}
