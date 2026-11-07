import { describe, expect, it } from 'vitest';
import {
	elapsed,
	localMidnight,
	localDay,
	longDate,
	longDay,
	monthLabel,
	relativeDay,
	relativeTime,
	shortDate,
	shortDay,
	shortTime,
	utcDay,
	ymd
} from './dates';

const at = (y: number, m: number, d: number, h = 0, min = 0) =>
	Math.floor(new Date(y, m - 1, d, h, min).getTime() / 1000);

describe('dates', () => {
	it('formats YYYY-MM-DD in local time', () => {
		expect(ymd(at(2026, 3, 7, 23, 59))).toBe('2026-03-07');
	});

	it('reads a day-only time as its UTC day, wherever it is read', () => {
		expect(utcDay(Date.UTC(2026, 9, 23) / 1000)).toBe('2026-10-23');
	});

	it('counts a day’s age in calendar days', () => {
		const now = at(2026, 10, 26, 0, 30);
		expect(relativeDay('2026-10-26', now)).toBe('today');
		expect(relativeDay('2026-10-23', now)).toBe('3d');
		expect(relativeDay('2026-10-01', now, 'en-US')).toBe('Oct 1');
	});

	it('gives local YYYY-MM-DD days', () => {
		const now = new Date(2026, 2, 7, 23, 59);
		expect(localDay(0, now)).toBe('2026-03-07');
		expect(localDay(7, now)).toBe('2026-02-28');
	});

	it('writes the long form of a day', () => {
		expect(longDay('2026-10-12', 'en-US')).toBe('Monday, October 12, 2026');
	});

	it('writes a short day, with the year only outside today’s', () => {
		expect(shortDay('2026-10-03', '2026-10-12', 'en-US')).toBe('Oct 3');
		expect(shortDay('2025-10-03', '2026-10-12', 'en-US')).toBe('Oct 3, 2025');
	});

	it('formats a zero-padded 24-hour time', () => {
		expect(shortTime(at(2026, 1, 1, 9, 5))).toBe('9:05 AM');
		expect(shortTime(at(2026, 1, 1, 0, 5))).toBe('12:05 AM');
		expect(shortTime(at(2026, 1, 1, 12, 0))).toBe('12:00 PM');
		expect(shortTime(at(2026, 1, 1, 21, 30))).toBe('9:30 PM');
	});

	it('groups by month', () => {
		expect(monthLabel('2026-11', 'en-US')).toBe('November 2026');
	});

	it('writes the long form with the weekday', () => {
		expect(longDate(at(2026, 10, 12), 'en-US')).toBe('Monday, October 12, 2026');
	});

	it('writes a short date, with the year only outside this one', () => {
		const now = at(2026, 10, 12, 15);
		expect(shortDate(at(2026, 10, 3), now, 'en-US')).toBe('Oct 3');
		expect(shortDate(at(2025, 10, 3), now, 'en-US')).toBe('Oct 3, 2025');
	});

	it('writes compact ages, then a short date after a week', () => {
		const now = at(2026, 10, 12, 15);
		expect(relativeTime(now, now)).toBe('now');
		expect(relativeTime(now + 30, now)).toBe('now');
		expect(relativeTime(at(2026, 10, 12, 14, 48), now)).toBe('12m');
		expect(relativeTime(at(2026, 10, 12, 9), now)).toBe('6h');
		expect(relativeTime(at(2026, 10, 10, 15), now)).toBe('2d');
		expect(relativeTime(at(2026, 10, 3), now, 'en-US')).toBe('Oct 3');
		expect(relativeTime(at(2025, 10, 3), now, 'en-US')).toBe('Oct 3, 2025');
	});

	it('finds local midnight', () => {
		const now = new Date(2026, 9, 12, 15, 30);
		expect(localMidnight(0, now)).toBe(at(2026, 10, 12));
		expect(localMidnight(1, now)).toBe(at(2026, 10, 11));
	});

	it('says how long something took, in its two largest units', () => {
		expect(elapsed(0)).toBe('<1s');
		expect(elapsed(45)).toBe('45s');
		expect(elapsed(192)).toBe('3m 12s');
		expect(elapsed(3900)).toBe('1h 5m');
	});
});
