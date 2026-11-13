import { describe, expect, it } from 'vitest';
import type { EventTiming } from '$lib/protocol/types';
import {
	addDays,
	atMinute,
	eventDays,
	isAllDayRow,
	layoutBars,
	layoutDay,
	moveByDays,
	moveTo,
	step,
	timeRange,
	viewDays,
	viewTitle
} from './days';

const timed = (day: string, from: number, to: number, toDay = day): { timing: EventTiming } => ({
	timing: {
		kind: 'timed',
		starts_at: atMinute(day, from),
		ends_at: atMinute(toDay, to),
		time_zone: 'UTC'
	}
});
const allDay = (start_date: string, end_date: string): { timing: EventTiming } => ({
	timing: { kind: 'allDay', start_date, end_date }
});

describe('viewDays', () => {
	it('starts a week on Sunday', () => {
		expect(viewDays('week', '2026-11-13')).toEqual([
			'2026-11-08',
			'2026-11-09',
			'2026-11-10',
			'2026-11-11',
			'2026-11-12',
			'2026-11-13',
			'2026-11-14'
		]);
	});

	it('shows six weeks for a month, from the Sunday on or before the 1st', () => {
		const days = viewDays('month', '2026-11-13');
		expect(days).toHaveLength(42);
		expect(days[0]).toBe('2026-11-01');
		expect(viewDays('month', '2026-12-20')[0]).toBe('2026-11-29');
	});
});

describe('step', () => {
	it('keeps the day of the month where the next month has it', () => {
		expect(step('month', '2026-01-31', 1)).toBe('2026-02-28');
		expect(step('month', '2026-03-15', -1)).toBe('2026-02-15');
	});

	it('steps a week by seven days and a day by one', () => {
		expect(step('week', '2026-12-29', 1)).toBe('2027-01-05');
		expect(step('day', '2026-03-01', -1)).toBe('2026-02-28');
	});
});

describe('viewTitle', () => {
	it('names a week across months and years in full', () => {
		expect(viewTitle('week', '2026-11-13', 'en-US')).toBe('Nov 8 – 14, 2026');
		expect(viewTitle('week', '2026-12-01', 'en-US')).toBe('Nov 29 – Dec 5, 2026');
		expect(viewTitle('week', '2026-12-31', 'en-US')).toBe('Dec 27, 2026 – Jan 2, 2027');
	});
});

describe('eventDays', () => {
	it('ends an event ending at midnight on the day before', () => {
		expect(eventDays(timed('2026-11-13', 22 * 60, 0, '2026-11-14'))).toEqual([
			'2026-11-13',
			'2026-11-13'
		]);
		expect(isAllDayRow(timed('2026-11-13', 22 * 60, 60, '2026-11-14'))).toBe(true);
	});
});

describe('timeRange', () => {
	it('names the period once when both ends share it', () => {
		const t = (from: number, to: number) => timed('2026-11-13', from, to).timing;
		expect(timeRange(t(9 * 60, 10 * 60 + 30))).toBe('9:00 – 10:30 AM');
		expect(timeRange(t(11 * 60 + 30, 13 * 60))).toBe('11:30 AM – 1:00 PM');
		expect(timeRange(allDay('2026-11-13', '2026-11-13').timing)).toBe('All day');
	});
});

describe('layoutDay', () => {
	it('puts overlapping events side by side and leaves the rest whole', () => {
		const a = timed('2026-11-13', 9 * 60, 10 * 60);
		const b = timed('2026-11-13', 9 * 60 + 30, 11 * 60);
		const c = timed('2026-11-13', 10 * 60, 10 * 60 + 30);
		const d = timed('2026-11-13', 14 * 60, 15 * 60);
		const placed = layoutDay([a, b, c, d], '2026-11-13');
		const of = (e: object) => placed.find((p) => p.event === e)!;
		expect([of(a).lane, of(b).lane, of(c).lane]).toEqual([0, 1, 0]);
		expect([of(a).lanes, of(b).lanes, of(c).lanes]).toEqual([2, 2, 2]);
		expect(of(d)).toMatchObject({ lane: 0, lanes: 1, start: 840, end: 900 });
	});

	it('leaves all-day events and those on other days out', () => {
		expect(
			layoutDay([allDay('2026-11-13', '2026-11-13'), timed('2026-11-14', 60, 120)], '2026-11-13')
		).toEqual([]);
	});
});

describe('layoutBars', () => {
	it('stacks bars that share a day and clips them to the days shown', () => {
		const days = viewDays('week', '2026-11-13');
		const long = allDay('2026-11-01', '2026-11-10');
		const short = allDay('2026-11-09', '2026-11-09');
		const later = allDay('2026-11-12', '2026-11-20');
		const bars = layoutBars([short, long, later], days);
		const of = (e: object) => bars.find((b) => b.event === e)!;
		expect(of(long)).toMatchObject({ from: 0, span: 3, row: 0 });
		expect(of(short)).toMatchObject({ from: 1, span: 1, row: 1 });
		expect(of(later)).toMatchObject({ from: 4, span: 3, row: 0 });
	});
});

describe('moving', () => {
	it('moves an all-day event by days and a timed one keeping its length', () => {
		expect(moveByDays(allDay('2026-11-30', '2026-12-01').timing, 2)).toMatchObject({
			start_date: '2026-12-02',
			end_date: '2026-12-03'
		});
		const t = timed('2026-11-13', 9 * 60, 10 * 60).timing as Extract<
			EventTiming,
			{ kind: 'timed' }
		>;
		expect(moveTo(t, addDays('2026-11-13', 1), 14 * 60)).toMatchObject({
			starts_at: atMinute('2026-11-14', 14 * 60),
			ends_at: atMinute('2026-11-14', 15 * 60)
		});
	});
});
