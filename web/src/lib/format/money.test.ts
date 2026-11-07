import { describe, expect, it } from 'vitest';
import { perMillion, usd } from './money';

describe('perMillion', () => {
	it('formats dollars to cents', () => {
		expect(perMillion(3)).toBe('$3.00');
		expect(perMillion(1.25)).toBe('$1.25');
		expect(perMillion(0)).toBe('$0.00');
	});

	it('keeps sub-cent prices visible', () => {
		expect(perMillion(0.004)).toBe('$0.004');
	});

	it('shows a dash for unknown prices', () => {
		expect(perMillion(null)).toBe('—');
		expect(perMillion(undefined)).toBe('—');
	});
});

describe('usd', () => {
	it('keeps sub-cent amounts visible', () => {
		expect(usd(0.0042)).toBe('$0.0042');
		expect(usd(1.5)).toBe('$1.50');
	});
});
