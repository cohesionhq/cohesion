import { describe, expect, it } from 'vitest';
import { EMOJI_GROUPS, hasEmoji, searchEmoji } from './emoji';

describe('searchEmoji', () => {
	it('matches word prefixes, not substrings', () => {
		const chars = searchEmoji('ca').map((e) => e.char);
		expect(chars).toContain('🐱');
		expect(chars).toContain('📅');
		expect(chars).not.toContain('🦆');
	});

	it('needs every term to match', () => {
		expect(searchEmoji('green heart').map((e) => e.char)).toEqual(['💚']);
	});

	it('finds nothing for a blank query', () => {
		expect(searchEmoji('  ')).toEqual([]);
	});
});

it('offers each emoji once, so the grid keys stay unique', () => {
	const chars = EMOJI_GROUPS.flatMap((g) => g.emoji.map((e) => e.char));
	expect(new Set(chars).size).toBe(chars.length);
});

it('recognises a pasted emoji the set lacks', () => {
	expect(hasEmoji('🦩')).toBe(true);
	expect(hasEmoji('flamingo')).toBe(false);
});
