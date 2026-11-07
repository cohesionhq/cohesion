import { describe, expect, it } from 'vitest';
import { parseDay } from './naturalDay';

// A Tuesday.
const today = '2026-10-13';
const parse = (text: string) => parseDay(text, today);

describe('parseDay', () => {
	it('reads relative words', () => {
		expect(parse('today')).toBe('2026-10-13');
		expect(parse(' Tomorrow ')).toBe('2026-10-14');
		expect(parse('tmr')).toBe('2026-10-14');
		expect(parse('yesterday')).toBe('2026-10-12');
	});

	it('takes the first weekday after today', () => {
		expect(parse('fri')).toBe('2026-10-16');
		expect(parse('next friday')).toBe('2026-10-16');
		expect(parse('tue')).toBe('2026-10-20');
		expect(parse('mon')).toBe('2026-10-19');
		expect(parse('weekend')).toBe('2026-10-17');
	});

	it('reads next week, month and year', () => {
		expect(parse('next week')).toBe('2026-10-19');
		expect(parse('next month')).toBe('2026-11-01');
		expect(parse('next year')).toBe('2027-01-01');
	});

	it('counts offsets', () => {
		expect(parse('in 3 days')).toBe('2026-10-16');
		expect(parse('+5')).toBe('2026-10-18');
		expect(parse('2w')).toBe('2026-10-27');
		expect(parse('in 1 month')).toBe('2026-11-13');
		expect(parse('1 year')).toBe('2027-10-13');
	});

	it('clamps a month step to the end of a shorter month', () => {
		expect(parseDay('1 month', '2026-01-31')).toBe('2026-02-28');
	});

	it('reads named months, rolling a past date into next year', () => {
		expect(parse('oct 20')).toBe('2026-10-20');
		expect(parse('20 October')).toBe('2026-10-20');
		expect(parse('Dec 1st')).toBe('2026-12-01');
		expect(parse('mar 3')).toBe('2027-03-03');
		expect(parse('mar 3, 2026')).toBe('2026-03-03');
	});

	it('reads numeric dates month first', () => {
		expect(parse('10/20')).toBe('2026-10-20');
		expect(parse('1/5')).toBe('2027-01-05');
		expect(parse('1/5/26')).toBe('2026-01-05');
		expect(parse('2026-11-02')).toBe('2026-11-02');
	});

	it('names no day for nonsense or impossible dates', () => {
		expect(parse('')).toBeNull();
		expect(parse('soon')).toBeNull();
		expect(parse('20')).toBeNull();
		expect(parse('feb 30')).toBeNull();
		expect(parse('2026-13-01')).toBeNull();
		expect(parse('3 mo days')).toBeNull();
	});
});
