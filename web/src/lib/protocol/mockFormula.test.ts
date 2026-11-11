import { describe, expect, it } from 'vitest';
import { formulaStarter } from '$lib/features/tables/formulas';
import type { PropertyDataType, DocumentFolderProperty } from './types';
import { compileMockFormula } from './mockFormula';

const row = { Title: 'Dune', Pages: 412, Rating: 4.5, 'Unit price': 2.5, Read: true, Empty: null };
const run = (source: string, input: Record<string, unknown> = row) => {
	const formula = compileMockFormula(source);
	if (!formula) throw new Error(`Not read: ${source}`);
	return formula(input);
};

describe('mock formulas', () => {
	it('runs arithmetic, comparisons and choices over the row', () => {
		expect(run('(row) => row.Pages * row["Unit price"]')).toBe(1030);
		expect(run('row => (row.Empty ?? 0) + 1')).toBe(1);
		expect(run('(row) => row.Pages > 300 ? "Long" : "Short"')).toBe('Long');
		expect(run("(row) => row.Rating.toFixed(1) + ' stars'")).toBe('4.5 stars');
		expect(run('(row) => !row.Read && row.Title !== null')).toBe(false);
		expect(run('(row) => Math.max(row.Pages, 500) - -1')).toBe(501);
		expect(run('(row) => row.Title.toUpperCase().length')).toBe(4);
	});

	it('fails on an empty value as JavaScript would', () => {
		expect(() => run('(row) => row.Empty.toFixed(1)')).toThrow(/toFixed/);
	});

	it('reads nothing outside its part of the language', () => {
		for (const source of [
			'',
			'row.Pages',
			'(r) => r.Pages',
			'(row) => fetch("https://example.com")',
			'(row) => { return 1; }',
			'(row) => row.Pages = 1'
		])
			expect(compileMockFormula(source)).toBeNull();
	});

	it('runs every starter the formula editor offers', () => {
		const property = (
			id: number,
			name: string,
			data_type: Exclude<PropertyDataType, 'choice'>
		): DocumentFolderProperty => ({
			id,
			folder_id: 1,
			name,
			data_type,
			sort_order: id,
			formula: null
		});
		const properties = [
			property(1, 'Title', 'text'),
			property(2, 'Pages', 'integer'),
			property(3, 'Unit price', 'currency'),
			property(4, 'Read', 'checkbox'),
			property(5, 'Finished', 'date')
		];
		const types: PropertyDataType[] = [
			'text',
			'integer',
			'decimal',
			'currency',
			'checkbox',
			'date'
		];
		for (const type of types) {
			for (const available of [properties, []]) {
				const source = formulaStarter(type, available);
				expect(compileMockFormula(source), source).not.toBeNull();
			}
		}
		expect(formulaStarter('currency', properties)).toBe(
			'(row) => (row.Pages ?? 0) * (row["Unit price"] ?? 0)'
		);
		expect(run(formulaStarter('currency', properties))).toBe(1030);
	});
});
