import { dayToDate, ymd } from '$lib/format/dates';

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];
const MONTHS = [
	'january',
	'february',
	'march',
	'april',
	'may',
	'june',
	'july',
	'august',
	'september',
	'october',
	'november',
	'december'
];

const UNITS: Record<string, 'day' | 'week' | 'month' | 'year'> = {
	d: 'day',
	day: 'day',
	days: 'day',
	w: 'week',
	wk: 'week',
	wks: 'week',
	week: 'week',
	weeks: 'week',
	m: 'month',
	mo: 'month',
	month: 'month',
	months: 'month',
	y: 'year',
	yr: 'year',
	year: 'year',
	years: 'year'
};

/** The entry of `names` that `word` abbreviates, with at least three letters. */
function nameIndex(names: string[], word: string): number {
	if (word.length < 3) return -1;
	return names.findIndex((name) => name.startsWith(word));
}

/** A "2026-10-12" day from parts, or null when they name no real date. */
function makeDay(year: number, month: number, date: number): string | null {
	const d = new Date(year, month - 1, date);
	if (d.getFullYear() !== year || d.getMonth() !== month - 1 || d.getDate() !== date) return null;
	return ymd(d.getTime() / 1000);
}

/** `day` moved by `count` units; a month or year step keeps the date where the month has it. */
function shift(day: string, count: number, unit: 'day' | 'week' | 'month' | 'year'): string {
	const d = dayToDate(day);
	if (unit === 'day') d.setDate(d.getDate() + count);
	if (unit === 'week') d.setDate(d.getDate() + 7 * count);
	if (unit === 'month' || unit === 'year') {
		const date = d.getDate();
		d.setDate(1);
		d.setMonth(d.getMonth() + (unit === 'year' ? 12 * count : count));
		const last = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();
		d.setDate(Math.min(date, last));
	}
	return ymd(d.getTime() / 1000);
}

/** The first `weekday` (0 is Sunday) after `today`. */
function nextWeekday(today: string, weekday: number): string {
	const ahead = (weekday - dayToDate(today).getDay() + 7) % 7 || 7;
	return shift(today, ahead, 'day');
}

/** A month and date with no year: this year's, or next year's once it has passed. */
function upcoming(today: string, month: number, date: number): string | null {
	const year = Number(today.slice(0, 4));
	const day = makeDay(year, month, date);
	if (day === null || day >= today) return day;
	return makeDay(year + 1, month, date);
}

/**
 * The day a short phrase names, as "2026-10-12", or null when it names
 * none. `today` is the local day phrases count from. Understands:
 * "today", "tomorrow", "yesterday"; a weekday ("fri", "next friday"), the
 * first one after today; "next week" (its Monday), "next month" (its 1st),
 * "weekend" (Saturday); "in 3 days", "2 weeks", "+5", "1m"; "oct 20",
 * "20 october", "oct 20 2027"; "10/20", "10/20/27"; and "2026-10-20".
 */
export function parseDay(text: string, today: string): string | null {
	const phrase = text.trim().toLowerCase().replace(/,/g, ' ').replace(/\s+/g, ' ');
	if (phrase === '') return null;

	if (phrase === 'today' || phrase === 'tod') return today;
	if (['tomorrow', 'tmr', 'tmrw', 'tom'].includes(phrase)) return shift(today, 1, 'day');
	if (phrase === 'yesterday') return shift(today, -1, 'day');
	if (phrase === 'next week') return nextWeekday(today, 1);
	if (phrase === 'next month') {
		const next = dayToDate(shift(today, 1, 'month'));
		return makeDay(next.getFullYear(), next.getMonth() + 1, 1);
	}
	if (phrase === 'next year') return makeDay(Number(today.slice(0, 4)) + 1, 1, 1);
	if (phrase === 'weekend' || phrase === 'this weekend') return nextWeekday(today, 6);

	const weekday = nameIndex(WEEKDAYS, phrase.replace(/^(next|this|on) /, ''));
	if (weekday !== -1) return nextWeekday(today, weekday);

	// "in 3 days", "3 weeks", "+5", "2w"; a bare "+N" counts days.
	const offset = /^(?:in )?(\+)?(\d{1,3}) ?([a-z]*)$/.exec(phrase);
	if (offset && (offset[1] || offset[3] in UNITS)) {
		const unit = offset[3] === '' ? 'day' : UNITS[offset[3]];
		if (unit) return shift(today, Number(offset[2]), unit);
	}

	const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(phrase);
	if (iso) return makeDay(Number(iso[1]), Number(iso[2]), Number(iso[3]));

	// Month first, as in "10/20" and "10/20/27".
	const numeric = /^(\d{1,2})\/(\d{1,2})(?:\/(\d{2}|\d{4}))?$/.exec(phrase);
	if (numeric) {
		const [month, date] = [Number(numeric[1]), Number(numeric[2])];
		if (numeric[3] === undefined) return upcoming(today, month, date);
		const year = Number(numeric[3]) + (numeric[3].length === 2 ? 2000 : 0);
		return makeDay(year, month, date);
	}

	// "oct 20", "october 20 2027", "20 oct".
	const monthFirst = /^([a-z]+) (\d{1,2})(?:st|nd|rd|th)?(?: (\d{4}))?$/.exec(phrase);
	const dateFirst = /^(\d{1,2})(?:st|nd|rd|th)? ([a-z]+)(?: (\d{4}))?$/.exec(phrase);
	const named = monthFirst
		? { month: monthFirst[1], date: monthFirst[2], year: monthFirst[3] }
		: dateFirst && { month: dateFirst[2], date: dateFirst[1], year: dateFirst[3] };
	if (named) {
		const month = nameIndex(MONTHS, named.month) + 1;
		if (month === 0) return null;
		const date = Number(named.date);
		return named.year ? makeDay(Number(named.year), month, date) : upcoming(today, month, date);
	}

	return null;
}
