import { describe, expect, it } from 'vitest';
import { greeting } from './greeting';

describe('greeting', () => {
	it('names the user in every line at every hour', () => {
		for (let hour = 0; hour < 24; hour++)
			for (const pick of [0, 0.3, 0.6, 0.99]) expect(greeting(hour, 'Ada', pick)).toContain('Ada');
	});

	it('follows the time of day', () => {
		expect(greeting(9, 'Jo', 0)).toBe('Good morning, Jo');
		expect(greeting(14, 'Jo', 0)).toBe('Good afternoon, Jo');
		expect(greeting(19, 'Jo', 0)).toBe('Good evening, Jo');
		expect(greeting(2, 'Jo', 0.3)).toBe('Late night, Jo?');
	});

	it('falls back to a plain greeting without a name', () => {
		expect(greeting(9, null, 0.5)).toBe('Good morning');
	});
});
