// Unix seconds to display strings, all in local time.

function toDate(seconds: number): Date {
	return new Date(seconds * 1000);
}

const pad = (n: number) => String(n).padStart(2, '0');

/** "Monday, October 12, 2026" */
export function longDate(seconds: number, locale?: string): string {
	return toDate(seconds).toLocaleDateString(locale, {
		weekday: 'long',
		year: 'numeric',
		month: 'long',
		day: 'numeric'
	});
}

/** "9:05 AM": every time the UI shows is on a 12-hour clock, whatever the locale's habit. */
export function shortTime(seconds: number): string {
	const d = toDate(seconds);
	const period = d.getHours() < 12 ? 'AM' : 'PM';
	return `${d.getHours() % 12 || 12}:${pad(d.getMinutes())} ${period}`;
}

/** "2026-10-12", the form journal titles use. */
export function ymd(seconds: number): string {
	const d = toDate(seconds);
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** The local day `daysAgo` days before `now`, as "2026-10-12". */
export function localDay(daysAgo = 0, now: Date = new Date()): string {
	return ymd(localMidnight(daysAgo, now));
}

/** The UTC day of `seconds`, as "2026-10-12": the day a time known only to the day names. */
export function utcDay(seconds: number): string {
	return toDate(seconds).toISOString().slice(0, 10);
}

/** A "2026-10-12" day as a `Date` at its local midnight, for formatting. */
export function dayToDate(day: string): Date {
	const [year, month, date] = day.split('-').map(Number);
	return new Date(year, month - 1, date);
}

/** "Monday, October 12, 2026" for a "2026-10-12" day. */
export function longDay(day: string, locale?: string): string {
	return longDate(dayToDate(day).getTime() / 1000, locale);
}

/** "October 2026", the heading for a "2026-10" month. */
export function monthLabel(key: string, locale?: string): string {
	const [year, month] = key.split('-').map(Number);
	return new Date(year, month - 1, 1).toLocaleDateString(locale, {
		year: 'numeric',
		month: 'long'
	});
}

/** Local midnight `daysAgo` days before `now`, in unix seconds. */
export function localMidnight(daysAgo = 0, now: Date = new Date()): number {
	const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - daysAgo);
	return Math.floor(d.getTime() / 1000);
}

export function nowSeconds(): number {
	return Math.floor(Date.now() / 1000);
}

/** "Oct 3", or "Oct 3, 2025" outside the year of `now` (unix seconds). */
export function shortDate(seconds: number, now = nowSeconds(), locale?: string): string {
	const sameYear = toDate(seconds).getFullYear() === toDate(now).getFullYear();
	return toDate(seconds).toLocaleDateString(locale, {
		month: 'short',
		day: 'numeric',
		...(sameYear ? {} : { year: 'numeric' })
	});
}

/** "Oct 3", or "Oct 3, 2025" outside the year of `today`; both "2026-10-03" days. */
export function shortDay(day: string, today: string, locale?: string): string {
	return dayToDate(day).toLocaleDateString(locale, {
		month: 'short',
		day: 'numeric',
		...(day.slice(0, 4) === today.slice(0, 4) ? {} : { year: 'numeric' })
	});
}

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** A compact age: "now", "5m", "3h", "2d" within the week, then `shortDate`. */
export function relativeTime(seconds: number, now = nowSeconds(), locale?: string): string {
	const age = Math.max(0, now - seconds);
	if (age < MINUTE) return 'now';
	if (age < HOUR) return `${Math.floor(age / MINUTE)}m`;
	if (age < DAY) return `${Math.floor(age / HOUR)}h`;
	if (age < 7 * DAY) return `${Math.floor(age / DAY)}d`;
	return shortDate(seconds, now, locale);
}

/** `relativeTime` for a "2026-10-12" day: "today", "2d" within the week, then `shortDay`. */
export function relativeDay(day: string, now = nowSeconds(), locale?: string): string {
	const today = localDay(0, toDate(now));
	const days = Math.round((dayToDate(today).getTime() - dayToDate(day).getTime()) / (DAY * 1000));
	if (days <= 0) return 'today';
	if (days < 7) return `${days}d`;
	return shortDay(day, today, locale);
}

/** `relativeTime` for running text: "just now", "5m ago", "2d ago", then `shortDate`. */
export function ago(seconds: number, now = nowSeconds(), locale?: string): string {
	const age = now - seconds;
	if (age < MINUTE) return 'just now';
	const short = relativeTime(seconds, now, locale);
	return age < 7 * DAY ? `${short} ago` : short;
}

/** How long something took, from whole seconds: "<1s", "45s", "3m 12s", "1h 5m". */
export function elapsed(seconds: number): string {
	if (seconds < 1) return '<1s';
	if (seconds < MINUTE) return `${seconds}s`;
	if (seconds < HOUR) return `${Math.floor(seconds / MINUTE)}m ${seconds % MINUTE}s`;
	return `${Math.floor(seconds / HOUR)}h ${Math.floor((seconds % HOUR) / MINUTE)}m`;
}
