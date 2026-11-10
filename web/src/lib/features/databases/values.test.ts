import { describe, expect, it } from 'vitest';
import type { DatabaseProperty, DatabaseRowEntry } from '$lib/protocol/types';
import {
	conditionChoices,
	filterText,
	formatValue,
	inputText,
	parseInput,
	rowLabel,
	sortDirections
} from './values';

const property = (
	id: number,
	name: string,
	data_type: DatabaseProperty['data_type']
): DatabaseProperty => ({ id, database_id: 1, name, data_type, sort_order: id, formula: null });

describe('database values', () => {
	it('formats each type for a cell', () => {
		expect(formatValue({ type: 'text', value: 'Dune' })).toBe('Dune');
		expect(formatValue({ type: 'integer', value: 12345 }, '2026-11-09', 'en-US')).toBe('12,345');
		expect(formatValue({ type: 'decimal', value: 3.14159 }, '2026-11-09', 'en-US')).toBe('3.14159');
		expect(formatValue({ type: 'currency', value: 1299.5 }, '2026-11-09', 'en-US')).toBe(
			'1,299.50'
		);
		expect(formatValue({ type: 'date', value: '2026-10-03' }, '2026-11-09', 'en-US')).toBe('Oct 3');
		expect(formatValue({ type: 'date', value: '2025-10-03' }, '2026-11-09', 'en-US')).toBe(
			'Oct 3, 2025'
		);
	});

	it('reads an editor’s text as a value, blank as none', () => {
		expect(parseInput('text', 'Dune')).toEqual({ value: { type: 'text', value: 'Dune' } });
		expect(parseInput('text', '')).toEqual({ value: null });
		expect(parseInput('integer', ' 42 ')).toEqual({ value: { type: 'integer', value: 42 } });
		expect(parseInput('integer', '4.2')).toEqual({ error: 'Enter a whole number.' });
		expect(parseInput('decimal', '4.25')).toEqual({ value: { type: 'decimal', value: 4.25 } });
		expect(parseInput('currency', 'ten')).toEqual({ error: 'Enter a number.' });
		expect(parseInput('currency', '')).toEqual({ value: null });
		expect(parseInput('date', '2026-11-09')).toEqual({
			value: { type: 'date', value: '2026-11-09' }
		});
		expect(inputText({ type: 'currency', value: 12.5 })).toBe('12.5');
		expect(inputText(null)).toBe('');
	});

	it('offers the conditions each type allows', () => {
		const ids = (type: DatabaseProperty['data_type']) => conditionChoices(type).map((c) => c.id);
		expect(ids('text')).toContain('contains');
		expect(ids('integer')).not.toContain('contains');
		expect(ids('date')).toContain('lessThan');
		expect(ids('checkbox')).toEqual(['checked', 'unchecked']);
		expect(ids('text')).not.toContain('greaterThan');
	});

	it('words a filter for its pill', () => {
		const pages = property(1, 'Pages', 'integer');
		const author = property(2, 'Author', 'text');
		const read = property(3, 'Read', 'checkbox');
		expect(
			filterText(
				{
					property_id: 1,
					condition: { kind: 'greaterThan', value: { type: 'integer', value: 300 } }
				},
				pages
			)
		).toBe('Pages is greater than 300');
		expect(
			filterText({ property_id: 2, condition: { kind: 'contains', text: 'Le Guin' } }, author)
		).toBe('Author contains “Le Guin”');
		expect(filterText({ property_id: 2, condition: { kind: 'isEmpty' } }, author)).toBe(
			'Author is empty'
		);
		const unchecked = conditionChoices('checkbox')[1].condition(null, '');
		expect(filterText({ property_id: 3, condition: unchecked }, read)).toBe('Read isn’t checked');
		expect(sortDirections('text')).toEqual(['A to Z', 'Z to A']);
	});

	it('names a row by its first text property', () => {
		const properties = [property(1, 'Pages', 'integer'), property(2, 'Title', 'text')];
		const row = (cells: DatabaseRowEntry['cells']): DatabaseRowEntry => ({
			id: 1,
			database_id: 1,
			document_id: null,
			cells
		});
		expect(
			rowLabel(row([{ property_id: 2, value: { type: 'text', value: 'Dune' } }]), properties)
		).toBe('Dune');
		expect(rowLabel(row([]), properties)).toBe('Untitled');
	});
});
