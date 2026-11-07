import { describe, expect, it } from 'vitest';
import { initials } from './initials';

describe('initials', () => {
	it('takes the first and last words’ first letters', () => {
		expect(initials('Ada Lovelace')).toBe('AL');
		expect(initials('  ada  king lovelace ')).toBe('AL');
		expect(initials('Ada')).toBe('A');
		expect(initials('Élodie')).toBe('É');
	});

	it('is empty without a name', () => {
		expect(initials(null)).toBe('');
		expect(initials('   ')).toBe('');
	});
});
