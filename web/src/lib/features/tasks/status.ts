import type { TaskStatus } from '$lib/protocol/types';

/** What each status is called. */
export const STATUSES: Record<TaskStatus, string> = {
	incomplete: 'Open',
	completed: 'Done',
	cancelled: 'Cancelled'
};
