// Settings › Storage's breakdown of the database: each area the daemon
// measured, with its name, colour and share of the file, and free space last,
// so the shares add up. The bar and the rows draw from this one list.

import type { DatabaseSize, StorageArea } from '$lib/protocol/types';
import { agentFill } from '$lib/ui/colors';

export interface StorageRow {
	label: string;
	help?: string;
	/** The `bg-*` class of its colour in the bar, its legend and its row. */
	fill: string;
	bytes: number;
}

const AREAS: Record<StorageArea, Omit<StorageRow, 'bytes'>> = {
	agentChats: {
		label: 'Agent chats',
		help: 'Messages, the model threads behind them, and their images.',
		fill: agentFill.blue
	},
	actorSessions: {
		label: 'Actor sessions',
		help: 'Transcripts and their images.',
		fill: agentFill.purple
	},
	documentsAndTasks: {
		label: 'Documents and tasks',
		help: 'Journal entries, documents, tasks, folders, tags and saved views.',
		fill: agentFill.orange
	},
	feedsAndArticles: { label: 'Feeds and articles', fill: agentFill.green },
	resources: { label: 'Resources', fill: agentFill.teal },
	memories: { label: 'Memories', fill: agentFill.pink },
	rewindVersions: {
		label: 'Rewind versions',
		help: 'Earlier versions of tasks, documents and journal entries.',
		fill: agentFill.yellow
	},
	other: {
		label: 'Everything else',
		help: 'Workspace search, contacts, settings, extensions, models and spending.',
		fill: 'bg-fg-subtle'
	}
};

/** Each area in the order the daemon sent them, then free space. */
export function storageRows(size: DatabaseSize): StorageRow[] {
	return [
		...size.areas.map(({ area, bytes }) => ({ ...AREAS[area], bytes })),
		{
			label: 'Free space',
			help: 'Space Cohesion previously used that will be reused.',
			fill: 'bg-n4',
			bytes: size.free_bytes
		}
	];
}

/** `bytes` as a share of the file: "44%", "<1%" for a sliver. */
export function storageShare(bytes: number, total: number): string {
	if (total === 0 || bytes === 0) return '0%';
	const percent = (bytes / total) * 100;
	return percent < 1 ? '<1%' : `${Math.round(percent)}%`;
}
