// The calendar page's days: which a view shows, how it steps, what an
// event covers on each, and how a moved event's timing changes. Days are
// local "YYYY-MM-DD" strings; a timed event shows in local time. Weeks start
// on Sunday.

import type { CalendarEvent, EventTiming } from '$lib/protocol/types';
import { dayToDate, shortTime, ymd } from '$lib/format/dates';

export type CalendarView = 'month' | 'week' | 'day';

export const VIEWS: { id: CalendarView; label: string }[] = [
	{ id: 'month', label: 'Month' },
	{ id: 'week', label: 'Week' },
	{ id: 'day', label: 'Day' }
];

/** `view` as `?view=` spells it, or null when it names none. */
export function parseView(value: string | null): CalendarView | null {
	return VIEWS.some((v) => v.id === value) ? (value as CalendarView) : null;
}

/** Whether `value` is a "YYYY-MM-DD" day. */
export function isDay(value: string | null): value is string {
	return value !== null && /^\d{4}-\d{2}-\d{2}$/.test(value) && !isNaN(dayToDate(value).getTime());
}

/** Local midnight of `day`, in Unix seconds. */
export function dayStart(day: string): number {
	return dayToDate(day).getTime() / 1000;
}

export function addDays(day: string, n: number): string {
	const d = dayToDate(day);
	d.setDate(d.getDate() + n);
	return ymd(d.getTime() / 1000);
}

/** How many days `to` is after `from`. */
export function daysBetween(from: string, to: string): number {
	return Math.round((dayToDate(to).getTime() - dayToDate(from).getTime()) / 86_400_000);
}

/** The Sunday on or before `day`. */
export function weekStart(day: string): string {
	return addDays(day, -dayToDate(day).getDay());
}

/** The days `view` shows around `anchor`: a month's six weeks, a week, or the day. */
export function viewDays(view: CalendarView, anchor: string): string[] {
	if (view === 'day') return [anchor];
	const first = view === 'week' ? weekStart(anchor) : weekStart(`${anchor.slice(0, 8)}01`);
	return Array.from({ length: view === 'week' ? 7 : 42 }, (_, i) => addDays(first, i));
}

/** From the first day's midnight to the midnight after the last. */
export function rangeOf(days: string[]): { from: number; to: number } {
	return { from: dayStart(days[0]), to: dayStart(addDays(days[days.length - 1], 1)) };
}

/** The anchor a step back (-1) or forward (1) lands on. A month keeps its day where it can. */
export function step(view: CalendarView, anchor: string, by: -1 | 1): string {
	if (view === 'day') return addDays(anchor, by);
	if (view === 'week') return addDays(anchor, 7 * by);
	const d = dayToDate(anchor);
	const target = new Date(d.getFullYear(), d.getMonth() + by, 1);
	const last = new Date(target.getFullYear(), target.getMonth() + 1, 0).getDate();
	target.setDate(Math.min(d.getDate(), last));
	return ymd(target.getTime() / 1000);
}

/** The header's title: "November 2026", "Nov 8 – 14, 2026" or "Friday, November 13, 2026". */
export function viewTitle(view: CalendarView, anchor: string, locale?: string): string {
	const d = dayToDate(anchor);
	if (view === 'month') return d.toLocaleDateString(locale, { month: 'long', year: 'numeric' });
	if (view === 'day')
		return d.toLocaleDateString(locale, {
			weekday: 'long',
			month: 'long',
			day: 'numeric',
			year: 'numeric'
		});
	const days = viewDays('week', anchor);
	const [first, last] = [dayToDate(days[0]), dayToDate(days[6])];
	const month = (x: Date) => x.toLocaleDateString(locale, { month: 'short' });
	const year = last.getFullYear();
	if (first.getFullYear() !== year)
		return `${month(first)} ${first.getDate()}, ${first.getFullYear()} – ${month(last)} ${last.getDate()}, ${year}`;
	if (first.getMonth() !== last.getMonth())
		return `${month(first)} ${first.getDate()} – ${month(last)} ${last.getDate()}, ${year}`;
	return `${month(first)} ${first.getDate()} – ${last.getDate()}, ${year}`;
}

/** The first and last day `event` shows on. A timed event ending at midnight ends the day before. */
export function eventDays(event: Pick<CalendarEvent, 'timing'>): [string, string] {
	const t = event.timing;
	if (t.kind === 'allDay') return [t.start_date, t.end_date];
	const last = Math.max(t.starts_at, t.ends_at - 1);
	return [ymd(t.starts_at), ymd(last)];
}

/** Whether `event` shows on `day`. */
export function isOn(event: Pick<CalendarEvent, 'timing'>, day: string): boolean {
	const [first, last] = eventDays(event);
	return first <= day && day <= last;
}

/** Whether `event` sits in the all-day row: an all-day event, or a timed one spanning days. */
export function isAllDayRow(event: Pick<CalendarEvent, 'timing'>): boolean {
	if (event.timing.kind === 'allDay') return true;
	const [first, last] = eventDays(event);
	return first !== last;
}

/** "9:00 – 10:30 AM", "11:30 AM – 1:00 PM", or "All day". */
export function timeRange(timing: EventTiming): string {
	if (timing.kind === 'allDay') return 'All day';
	const [start, end] = [shortTime(timing.starts_at), shortTime(timing.ends_at)];
	const period = (t: string) => t.slice(-2);
	if (period(start) === period(end)) return `${start.slice(0, -3)} – ${end}`;
	return `${start} – ${end}`;
}

/** A timed event's place in one day column: minutes from midnight, and its lane. */
export interface Placed<T> {
	event: T;
	start: number;
	end: number;
	/** Which of `lanes` side by side it takes, from 0. */
	lane: number;
	lanes: number;
}

/**
 * Lays out the timed events of `day` in its column: each by its minutes,
 * events that overlap side by side in lanes, a cluster of overlapping events
 * sharing one lane count. An event shorter than 15 minutes takes 15.
 */
export function layoutDay<T extends Pick<CalendarEvent, 'timing'>>(
	events: T[],
	day: string
): Placed<T>[] {
	const midnight = dayStart(day);
	const placed = events
		.filter((e) => e.timing.kind === 'timed' && isOn(e, day) && !isAllDayRow(e))
		.map((event) => {
			const t = event.timing as Extract<EventTiming, { kind: 'timed' }>;
			const start = Math.max(0, Math.round((t.starts_at - midnight) / 60));
			const end = Math.min(1440, Math.max(start + 15, Math.round((t.ends_at - midnight) / 60)));
			return { event, start, end, lane: 0, lanes: 1 };
		})
		.sort((a, b) => a.start - b.start || b.end - a.end);
	let cluster: Placed<T>[] = [];
	let clusterEnd = -1;
	const close = () => {
		const lanes = Math.max(1, ...cluster.map((p) => p.lane + 1));
		for (const p of cluster) p.lanes = lanes;
		cluster = [];
	};
	for (const p of placed) {
		if (p.start >= clusterEnd) close();
		const taken = new Set(cluster.filter((q) => q.end > p.start).map((q) => q.lane));
		while (taken.has(p.lane)) p.lane++;
		cluster.push(p);
		clusterEnd = Math.max(clusterEnd, p.end);
	}
	close();
	return placed;
}

/** An all-day row's bar: the columns it spans in `days`, and its row. */
export interface Bar<T> {
	event: T;
	/** The first column, from 0. */
	from: number;
	/** How many columns it spans. */
	span: number;
	row: number;
}

/** Lays out the all-day row's events over `days` in rows, longest first, none overlapping. */
export function layoutBars<T extends Pick<CalendarEvent, 'timing'>>(
	events: T[],
	days: string[]
): Bar<T>[] {
	const [first, last] = [days[0], days[days.length - 1]];
	const bars = events
		.filter(isAllDayRow)
		.map((event) => {
			const [start, end] = eventDays(event);
			if (end < first || start > last) return null;
			const from = Math.max(0, daysBetween(first, start));
			const to = Math.min(days.length - 1, daysBetween(first, end));
			return { event, from, span: to - from + 1, row: 0 };
		})
		.filter((b) => b !== null)
		.sort((a, b) => a.from - b.from || b.span - a.span);
	const rows: number[] = [];
	for (const bar of bars) {
		let row = rows.findIndex((end) => end < bar.from);
		if (row === -1) row = rows.length;
		rows[row] = bar.from + bar.span - 1;
		bar.row = row;
	}
	return bars;
}

/** `timing` moved by `days` whole days, the time of day kept. */
export function moveByDays(timing: EventTiming, days: number): EventTiming {
	if (timing.kind === 'allDay')
		return {
			...timing,
			start_date: addDays(timing.start_date, days),
			end_date: addDays(timing.end_date, days)
		};
	const start = new Date(timing.starts_at * 1000);
	start.setDate(start.getDate() + days);
	const starts_at = start.getTime() / 1000;
	return { ...timing, starts_at, ends_at: starts_at + (timing.ends_at - timing.starts_at) };
}

/** A timed `timing` moved to start on `day` at `minute`, its length kept. */
export function moveTo(
	timing: Extract<EventTiming, { kind: 'timed' }>,
	day: string,
	minute: number
): EventTiming {
	const starts_at = atMinute(day, minute);
	return { ...timing, starts_at, ends_at: starts_at + (timing.ends_at - timing.starts_at) };
}

/** `minute` minutes into `day`, in Unix seconds, through any change of clock that day. */
export function atMinute(day: string, minute: number): number {
	const d = dayToDate(day);
	d.setMinutes(minute);
	return d.getTime() / 1000;
}

/** Minutes since `day`'s midnight of `seconds`. */
export function minuteOf(seconds: number): number {
	const d = new Date(seconds * 1000);
	return d.getHours() * 60 + d.getMinutes();
}

/** `minute` to the nearest step of `by` minutes. */
export function snap(minute: number, by = 15): number {
	return Math.round(minute / by) * by;
}

/** "9 AM" for an hour of the grid's gutter, "Noon" at 12. */
export function hourLabel(hour: number): string {
	if (hour === 12) return 'Noon';
	return `${hour % 12 || 12} ${hour < 12 ? 'AM' : 'PM'}`;
}

/** The browser's time zone, which new events are written in. */
export function localZone(): string {
	return Intl.DateTimeFormat().resolvedOptions().timeZone;
}
