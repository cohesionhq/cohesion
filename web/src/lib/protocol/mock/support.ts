// Small helpers the mock's seed data and handlers share: days and times,
// search words, paging and the order of tasks.

import { ProtocolError } from '../errors';
import type { DayFilter, Task, TaskPriority, TaskSort, Vocabulary } from '../types';
import vocabulary from '../vocabulary.json';

/** The daemon's words, as its `hello` would carry them. */
export const MOCK_VOCABULARY = vocabulary as Vocabulary;

export const DAY = 86_400;

export function localMidnight(daysAgo: number, now = new Date()): number {
	const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() - daysAgo);
	return Math.floor(date.getTime() / 1000);
}

export function ymd(seconds: number): string {
	const d = new Date(seconds * 1000);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

/** The local day `days` days from today, as "2026-10-12". */
export function dayIn(days: number): string {
	const d = new Date();
	return ymd(new Date(d.getFullYear(), d.getMonth(), d.getDate() + days).getTime() / 1000);
}

/** Whether `day` matches `filter`, as the daemon's day filters do: a task with no day never matches. */
export function onDay(day: string | null, filter: DayFilter): boolean {
	if (day === null) return false;
	if (typeof filter === 'string') return day === filter;
	if (Array.isArray(filter)) return filter.includes(day);
	return (
		(filter.before == null || day < filter.before) && (filter.after == null || day > filter.after)
	);
}

/** What the daemon titles an article whose page gives no title: its path's last part, else its host. */
/** One page of `rows`, sized as the daemon sizes it: 50 when unasked, never over 500. */
/** A search query's words, lowercased, as the full-text index splits them. */
export function searchWords(query: string): string[] {
	return (query.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
}

/** Whether `text` has a word starting with the word asked about: the index matches prefixes. */
export function starts(text: string): (word: string) => boolean {
	const tokens = text.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
	return (word) => tokens.some((token) => token.startsWith(word));
}

export function paged<T>(
	rows: T[],
	offset: number,
	limit: number | null
): { rows: T[]; total: number } {
	const size = Math.min(limit ?? 50, 500);
	return { rows: rows.slice(offset, offset + size), total: rows.length };
}

export function fallbackTitle(url: URL): string {
	return url.pathname.split('/').findLast((part) => part !== '') ?? url.host;
}

export const PRIORITY_RANK: Record<TaskPriority, number> = {
	none: 0,
	low: 1,
	medium: 2,
	high: 3,
	urgent: 4
};

// The daemon's task order: ties by id, and tasks missing a size or date last either way.
export function compareTasks(a: Task, b: Task, sort: TaskSort, descending: boolean): number {
	const flip = descending ? -1 : 1;
	let order: number;
	if (sort === 'title') {
		order = flip * a.title.toLowerCase().localeCompare(b.title.toLowerCase());
	} else if (sort === 'priority') {
		order = flip * (PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority]);
	} else if (sort === 'size') {
		if (a.size === null || b.size === null)
			order = (a.size === null ? 1 : 0) - (b.size === null ? 1 : 0);
		else order = flip * (a.size - b.size);
	} else if (sort === 'startDate' || sort === 'dueDate') {
		const [x, y] = sort === 'startDate' ? [a.start_date, b.start_date] : [a.due_date, b.due_date];
		if (x === null || y === null) order = (x === null ? 1 : 0) - (y === null ? 1 : 0);
		else order = flip * x.localeCompare(y);
	} else {
		order = flip * (a.id - b.id);
	}
	return order || a.id - b.id;
}

/** Refuses an export or import: mock mode has no database file to move. */
export function noTransfers(): never {
	throw new ProtocolError('badRequest', 'Mock mode has no data to export or import.');
}
