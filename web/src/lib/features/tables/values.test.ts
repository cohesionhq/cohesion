import { describe, expect, it } from 'vitest';
import type { DocumentFolderProperty, TableRow } from '$lib/protocol/types';
import {
	TITLE_COLUMN,
	columnInfo,
	conditionChoices,
	filterText,
	formatValue,
	holdsValues,
	inputText,
	parseInput,
	propertyColumn,
	sortDirections
} from './values';

const property = (
	id: number,
	name: string,
	data_type: Exclude<DocumentFolderProperty['data_type'], 'choice'>,
	formula: string | null = null
): DocumentFolderProperty => ({
	id,
	folder_id: 1,
	name,
	data_type,
	sort_order: id,
	formula: formula === null ? null : { source: formula, update_mode: 'whenChanged', error: null }
});

/** A choice property whose options are `names`, with ids from 10. */
const choice = (id: number, name: string, names: string[]): DocumentFolderProperty => ({
	id,
	folder_id: 1,
	name,
	data_type: 'choice',
	options: names.map((option, i) => ({
		id: 10 + i,
		property_id: id,
		name: option,
		color: i === 0 ? 'green' : null,
		sort_order: i
	})),
	sort_order: id,
	formula: null
});

describe('table values', () => {
	it('formats each type for a cell', () => {
		expect(formatValue({ type: 'text', value: 'Dune' })).toBe('Dune');
		expect(formatValue({ type: 'integer', value: 12345 }, [], '2026-11-09', 'en-US')).toBe(
			'12,345'
		);
		expect(formatValue({ type: 'decimal', value: 3.14159 }, [], '2026-11-09', 'en-US')).toBe(
			'3.14159'
		);
		expect(formatValue({ type: 'currency', value: 1299.5 }, [], '2026-11-09', 'en-US')).toBe(
			'1,299.50'
		);
		expect(formatValue({ type: 'date', value: '2026-10-03' }, [], '2026-11-09', 'en-US')).toBe(
			'Oct 3'
		);
		expect(formatValue({ type: 'date', value: '2025-10-03' }, [], '2026-11-09', 'en-US')).toBe(
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
		const ids = (type: DocumentFolderProperty['data_type']) =>
			conditionChoices(type).map((c) => c.id);
		expect(ids('text')).toContain('contains');
		expect(ids('integer')).not.toContain('contains');
		expect(ids('date')).toContain('lessThan');
		expect(ids('checkbox')).toEqual(['checked', 'unchecked']);
		expect(ids('text')).not.toContain('greaterThan');
		// A title is never empty.
		expect(conditionChoices('text', true).map((c) => c.id)).toEqual([
			'contains',
			'notContains',
			'equals',
			'notEquals'
		]);
	});

	it('words a filter for its pill', () => {
		const pages = property(1, 'Pages', 'integer');
		const author = property(2, 'Author', 'text');
		const read = property(3, 'Read', 'checkbox');
		expect(
			filterText(
				{
					column: { kind: 'property', property_id: 1 },
					condition: { kind: 'greaterThan', value: { type: 'integer', value: 300 } }
				},
				propertyColumn(pages)
			)
		).toBe('Pages is greater than 300');
		expect(
			filterText(
				{
					column: { kind: 'property', property_id: 2 },
					condition: { kind: 'contains', text: 'Le Guin' }
				},
				propertyColumn(author)
			)
		).toBe('Author contains “Le Guin”');
		expect(
			filterText(
				{ column: { kind: 'property', property_id: 2 }, condition: { kind: 'isEmpty' } },
				propertyColumn(author)
			)
		).toBe('Author is empty');
		const unchecked = conditionChoices('checkbox')[1].condition(null, '');
		expect(
			filterText(
				{ column: { kind: 'property', property_id: 3 }, condition: unchecked },
				propertyColumn(read)
			)
		).toBe('Read isn’t checked');
		expect(sortDirections('text')).toEqual(['A to Z', 'Z to A']);
	});

	it('words a filter on the title', () => {
		expect(
			filterText(
				{ column: { kind: 'title' }, condition: { kind: 'contains', text: 'Dune' } },
				TITLE_COLUMN
			)
		).toBe('Title contains “Dune”');
		const properties = [property(1, 'Pages', 'integer')];
		expect(columnInfo({ kind: 'title' }, properties)).toBe(TITLE_COLUMN);
		expect(columnInfo({ kind: 'property', property_id: 1 }, properties)?.name).toBe('Pages');
		expect(columnInfo({ kind: 'property', property_id: 9 }, properties)).toBeUndefined();
	});

	it('reads a choice as its option’s name', () => {
		const status = choice(4, 'Status', ['To read', 'Read']);
		const column = propertyColumn(status);
		expect(column.options.map((o) => o.name)).toEqual(['To read', 'Read']);
		expect(formatValue({ type: 'choice', value: 11 }, column.options)).toBe('Read');
		// An option that's gone reads as nothing.
		expect(formatValue({ type: 'choice', value: 99 }, column.options)).toBe('');
		expect(parseInput('choice', '11')).toEqual({ value: { type: 'choice', value: 11 } });
		expect(parseInput('choice', '')).toEqual({ value: null });
		expect(conditionChoices('choice').map((c) => c.id)).toEqual([
			'equals',
			'notEquals',
			'isEmpty',
			'isNotEmpty'
		]);
		expect(
			filterText(
				{
					column: { kind: 'property', property_id: 4 },
					condition: { kind: 'notEquals', value: { type: 'choice', value: 10 } }
				},
				column
			)
		).toBe('Status isn’t “To read”');
		expect(sortDirections('choice')).toEqual(['First option first', 'Last option first']);
		expect(propertyColumn(property(1, 'Pages', 'integer')).options).toEqual([]);
	});

	it('says whether a row holds a value set by hand, which leaving the table loses', () => {
		const properties = [
			property(1, 'Pages', 'integer'),
			property(2, 'Double', 'integer', '(row) => row.Pages * 2')
		];
		const row = (cells: TableRow['cells']): TableRow => ({
			document_id: 1,
			folder_id: 1,
			title: 'Dune',
			cells
		});
		expect(holdsValues(row([]), properties)).toBe(false);
		// A formula's output alone holds nothing set by hand.
		expect(
			holdsValues(row([{ property_id: 2, value: { type: 'integer', value: 4 } }]), properties)
		).toBe(false);
		expect(
			holdsValues(row([{ property_id: 1, value: { type: 'integer', value: 2 } }]), properties)
		).toBe(true);
	});
});
