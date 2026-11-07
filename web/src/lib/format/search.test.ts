import { describe, expect, it } from 'vitest';
import { matchingLine, prefixQuery } from './search';

describe('prefixQuery', () => {
	it('prefix-matches every word', () => {
		expect(prefixQuery('garden pl')).toBe('"garden"* "pl"*');
	});

	it('drops punctuation FTS5 would read as syntax', () => {
		expect(prefixQuery('e-mail: "re')).toBe('"e"* "mail"* "re"*');
		expect(prefixQuery('-(*')).toBeNull();
	});

	it('keeps letters outside ASCII', () => {
		expect(prefixQuery('café 2026')).toBe('"café"* "2026"*');
	});
});

describe('matchingLine', () => {
	const contents = '# Plans\n\nTomatoes by the fence.\nCall the Garden centre.';

	it('finds the line mentioning a typed word, whatever its case', () => {
		expect(matchingLine(contents, 'gard')).toBe('Call the Garden centre.');
	});

	it('falls back to the first line', () => {
		expect(matchingLine(contents, 'zzz')).toBe('# Plans');
	});
});
