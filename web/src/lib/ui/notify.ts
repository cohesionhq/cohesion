import type { Avatar } from './AgentAvatar.svelte';
import { toasts } from './toasts.svelte';

/**
 * Something that happened without the user doing it: a daemon notification, an agent's
 * message. A toast confirming what they just did (“Copied the task ID.”) isn't one.
 */
export interface AppNotification {
	message: string;
	/** A line above the message. */
	title?: string;
	/** The page's path to open when the notification is clicked. */
	href?: string;
	/** Who it is from, drawn as their avatar where the presenter can. */
	avatar?: Avatar;
}

declare global {
	interface Window {
		/**
		 * Set by a WebUI harness before the page loads to present notifications itself,
		 * as the system's local notifications. Without it, they are toasts.
		 */
		cohesionNotify?: (notification: AppNotification) => void;
		/** Set by the page, for a WebUI harness to open a clicked notification's `href`. */
		cohesionOpen?: (href: string) => void;
	}
}

/** Presents a notification: the harness's way if it has one, else a toast. */
export function notify(notification: AppNotification): void {
	const present = window.cohesionNotify;
	if (present) {
		try {
			present(notification);
			return;
		} catch {
			// A harness that fails to present it shouldn't lose it: it falls back to a toast.
		}
	}
	const { message, title, href, avatar } = notification;
	toasts.show(message, 'info', { title, href, avatar });
}

/**
 * Lets a WebUI harness open a clicked notification with `open`, which gets only the page's
 * own paths; returns the undo.
 */
export function openNotificationsWith(open: (href: string) => void): () => void {
	const opener = (href: string) => {
		if (href.startsWith('/') && !href.startsWith('//')) open(href);
	};
	window.cohesionOpen = opener;
	return () => {
		if (window.cohesionOpen === opener) delete window.cohesionOpen;
	};
}

const PREVIEW_LENGTH = 140;

/**
 * A message's text as a notification body: Markdown marks dropped, whitespace collapsed
 * and cut to a short length with an ellipsis. Empty when nothing readable is left.
 */
export function notificationPreview(text: string, max = PREVIEW_LENGTH): string {
	const plain = text
		.replace(/```[^\n]*\n?/g, ' ')
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/^\s{0,3}(?:#{1,6}|>|[-*+]|\d+\.)\s+/gm, '')
		.replace(/(\*\*|__|~~|[*_`])/g, '')
		.replace(/\s+/g, ' ')
		.trim();
	const chars = [...plain];
	return chars.length > max
		? `${chars
				.slice(0, max - 1)
				.join('')
				.trimEnd()}…`
		: plain;
}
