import { toProtocolError } from '$lib/protocol/errors';
import type { Avatar } from './AgentAvatar.svelte';

export interface ToastItem {
	id: number;
	message: string;
	tone: 'info' | 'error';
	/** A line above the message, as a notification's title. */
	title?: string;
	/** Where clicking the toast goes; without one, a click only dismisses it. */
	href?: string;
	/** A profile bubble before the message: the emoji (or the name's initial) on a disc of the color. */
	avatar?: Avatar;
}

export type ToastOptions = Pick<ToastItem, 'href' | 'avatar' | 'title'>;

const LIFETIME_MS = 4000;

/** Every lost-daemon failure (socket, HTTP, proxy 502) reads as this one message. */
export const UNREACHABLE_MESSAGE = 'Can’t reach Cohesion. Retrying…';

class Toasts {
	items = $state<ToastItem[]>([]);
	#nextId = 1;

	show(message: string, tone: ToastItem['tone'] = 'info', options: ToastOptions = {}): void {
		// A message already on screen (e.g. every request failing while the daemon is away) shows once.
		if (this.items.some((t) => t.message === message && t.title === options.title)) return;
		const id = this.#nextId++;
		this.items = [...this.items, { id, message, tone, ...options }];
		setTimeout(() => this.dismiss(id), LIFETIME_MS);
	}

	/** Shows a failed request's message; transport failures all share one. */
	error(error: unknown): void {
		const e = toProtocolError(error);
		this.show(e.code === 'disconnected' ? UNREACHABLE_MESSAGE : e.message, 'error');
	}

	dismiss(id: number): void {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();

/** Copies `text`, toasting `copied` ("Copied the task ID.") or why it failed. */
export function copyToClipboard(text: string, copied: string): void {
	navigator.clipboard.writeText(text).then(
		() => toasts.show(copied),
		() => toasts.show('Couldn’t copy to the clipboard.', 'error')
	);
}
