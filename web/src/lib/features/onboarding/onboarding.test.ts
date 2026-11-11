import { describe, expect, it } from 'vitest';
import { nextPage, previousPage, readiness } from './onboarding';

describe('onboarding pages', () => {
	it('steps forward to the last page and stops there', () => {
		expect(nextPage('workspace')).toBe('intelligence');
		expect(nextPage('agent')).toBe('done');
		expect(nextPage('done')).toBe('done');
	});

	it('steps back to the first page and no further', () => {
		expect(previousPage('intelligence')).toBe('workspace');
		expect(previousPage('workspace')).toBeNull();
	});
});

describe('readiness', () => {
	it('counts only the steps done, whatever their order', () => {
		expect(readiness(new Set())).toBe(0);
		expect(readiness(new Set(['agent', 'workspace']))).toBeCloseTo(2 / 3);
		expect(readiness(new Set(['workspace', 'intelligence', 'agent']))).toBe(1);
	});
});
