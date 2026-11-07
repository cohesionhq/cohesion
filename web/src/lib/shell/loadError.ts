// What a workspace page shows when its first load failed: a workspace that
// doesn't exist offers the picker; anything else offers a retry.

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { ProtocolError } from '$lib/protocol/errors';

export interface EmptyAction {
	message: string;
	actionLabel: string;
	onaction: () => void;
}

export function workspaceLoadError(error: ProtocolError, retry: () => unknown): EmptyAction {
	if (error.code === 'notFound')
		return {
			message: 'Workspace not found',
			actionLabel: 'Choose a workspace',
			onaction: () => void goto(resolve('/'), { replaceState: true })
		};
	return { message: error.message, actionLabel: 'Try again', onaction: () => void retry() };
}
