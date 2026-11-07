import { describe, expect, it } from 'vitest';
import { planOf, shortLimitLabel, usagePercent, usageTone } from './plan';

describe('usageTone', () => {
	it('warns past 80% and is in danger past 95%', () => {
		expect(usageTone(0.79)).toBe('calm');
		expect(usageTone(0.8)).toBe('warn');
		expect(usageTone(0.94)).toBe('warn');
		expect(usageTone(0.95)).toBe('danger');
		expect(usageTone(1.2)).toBe('danger');
	});
});

describe('usagePercent', () => {
	it('rounds to a whole percent', () => {
		expect(usagePercent(0.844)).toBe('84%');
		expect(usagePercent(0)).toBe('0%');
	});
});

describe('shortLimitLabel', () => {
	it('drops the word "limit" and what it leaves around it', () => {
		expect(shortLimitLabel('Weekly limit')).toBe('Weekly');
		expect(shortLimitLabel('5-hour limit')).toBe('5-hour');
		expect(shortLimitLabel('Limit: weekly (Opus)')).toBe('weekly (Opus)');
		expect(shortLimitLabel('Weekly LIMITS')).toBe('Weekly');
	});

	it('keeps "limit" inside another word', () => {
		expect(shortLimitLabel('Unlimited')).toBe('Unlimited');
	});

	it('keeps the label when nothing else would be left', () => {
		expect(shortLimitLabel('Limit')).toBe('Limit');
		expect(shortLimitLabel(' limit: ')).toBe(' limit: ');
	});
});

describe('planOf', () => {
	const weekly = { label: 'Weekly limit', used: 0.2 };
	const usage = [
		{ actor_ids: [1, 2], pool: { id: 'claude:a', name: 'a' }, plan: [weekly] },
		{ actor_ids: [3] }
	];

	it('gives every actor of a pool the pool plan', () => {
		expect(planOf(usage, 1)).toEqual([weekly]);
		expect(planOf(usage, 2)).toEqual([weekly]);
	});

	it('is empty for an actor reporting no plan or not listed', () => {
		expect(planOf(usage, 3)).toEqual([]);
		expect(planOf(usage, 9)).toEqual([]);
	});
});
