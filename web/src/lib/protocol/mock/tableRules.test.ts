// Runs the cases the daemon's own test runs
// (`the_shared_table_cases_hold`, cohesion-db), so the mock's copy of how a
// table filters and sorts can't drift from it. When a case fails only here,
// the mock is what's wrong.

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { createMock } from '../mock';
import type {
	DocumentFolderProperty,
	PropertyDataType,
	PropertyValue,
	TableColumn,
	TableCondition,
	TableFilter
} from '../types';

interface CaseValue {
	type: PropertyDataType;
	value: unknown;
}
interface Shared {
	properties: { name: string; type: PropertyDataType; options?: string[] }[];
	rows: { title: string; values: Record<string, CaseValue> }[];
	cases: {
		name: string;
		filters: { column: string; condition: { kind: string; value?: CaseValue; text?: string } }[];
		sort: { column: string; descending: boolean } | null;
		titles: string[];
	}[];
}

const shared: Shared = JSON.parse(
	readFileSync(
		new URL(
			'../../../../../cohesion-db/src/commands/document_value/table_cases.json',
			import.meta.url
		),
		'utf8'
	)
);

/** The shared table, built through the mock's own requests. */
async function table() {
	const { send } = createMock();
	const ws = (await send('createWorkspace', { title: 'Home' })).id;
	const folder = await send('createDocumentFolder', { workspace_id: ws, title: 'Books' });
	const at = { workspace_id: ws, folder_id: folder.id };
	const properties: DocumentFolderProperty[] = [];
	for (const { name, type, options } of shared.properties)
		properties.push(
			await send('createDocumentFolderProperty', {
				...at,
				name,
				data_type: type,
				options: (options ?? []).map((name) => ({ name }))
			})
		);
	const property = (name: string) => properties.find((p) => p.name === name)!;
	/** A case's value as the wire carries it: a choice by its option's id. */
	const value = (name: string, { type, value }: CaseValue): PropertyValue => {
		const p = property(name);
		if (type !== 'choice' || p.data_type !== 'choice') return { type, value } as PropertyValue;
		return { type, value: p.options.find((o) => o.name === value)!.id };
	};
	const column = (name: string): TableColumn =>
		name === '$title' ? { kind: 'title' } : { kind: 'property', property_id: property(name).id };
	for (const row of shared.rows) {
		const document = await send('createDocument', {
			workspace_id: ws,
			title: row.title,
			contents: '',
			folder_id: folder.id
		});
		for (const [name, written] of Object.entries(row.values))
			await send('setDocumentValue', {
				...at,
				document_id: document.id,
				property_id: property(name).id,
				value: value(name, written)
			});
	}
	return { send, at, value, column };
}

describe('the table cases shared with the daemon', () => {
	for (const shown of shared.cases)
		it(shown.name, async () => {
			const { send, at, value, column } = await table();
			const filters = shown.filters.map((filter): TableFilter => ({
				column: column(filter.column),
				condition: (filter.condition.value
					? { ...filter.condition, value: value(filter.column, filter.condition.value) }
					: filter.condition) as TableCondition
			}));
			const sort = shown.sort && {
				column: column(shown.sort.column),
				descending: shown.sort.descending
			};
			const rows = await send('listTableRows', { ...at, filters, sort });
			expect(rows.map((row) => row.title)).toEqual(shown.titles);
		});
});
