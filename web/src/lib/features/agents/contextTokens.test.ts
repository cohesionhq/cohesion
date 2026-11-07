import { describe, expect, it } from 'vitest';
import { contextTokens, contextTokensLabel } from './contextTokens';

describe('contextTokens', () => {
	it('is a dash in thousands with no live context, exact under a thousand, then in thousands', () => {
		expect(contextTokens(null)).toBe('—K');
		expect(contextTokens(999)).toBe('999');
		expect(contextTokens(18_400)).toBe('18K');
	});

	it('says unknown, not the dash, to a screen reader', () => {
		expect(contextTokensLabel(null)).toBe('Unknown');
		expect(contextTokensLabel(18_400)).toBe('18K');
	});
});
