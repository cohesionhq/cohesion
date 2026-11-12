import { describe, expect, it, vi } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { DocumentFolderProperty, TableRow } from '$lib/protocol/types';
import { createDocumentsStore } from './documents.svelte';
import { createTablesStore, rememberedSort } from './tables.svelte';

/** Each row's value for `property_id`, as the rows are listed. */
const column = (rows: TableRow[], property_id: number) =>
	rows.map((r) => r.cells.find((c) => c.property_id === property_id)?.value.value ?? null);

// The seed's tables: folder 4, the reading list (1 Author, 2 Pages, 3 Rating,
// 4 Price, 5 Read, 6 Finished, 7 Stars, a formula of Rating), and folder 5,
// the renovation budget (8 Supplier, 9 Quantity, 10 Unit price, 11 Total, a
// formula of the two computed continuously, 12 Ordered, 13 Delivery). Folder 1, Design, is plain.
// The reading list also has 14 Status, a choice of 1 To read, 2 Reading and 3 Finished.
const READING = 4;
const KITCHEN = 5;
const STATUS = 14;

/** The names of a choice's options, in order. */
const optionNames = (property: DocumentFolderProperty) =>
	property.data_type === 'choice' ? property.options.map((o) => o.name) : [];

describe('tables store', () => {
	it('opens a table with its properties and rows, and sets values from the reply', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, KITCHEN);
		expect(store.properties.value.map((p) => p.name)).toEqual([
			'Supplier',
			'Quantity',
			'Unit price',
			'Total',
			'Ordered',
			'Delivery'
		]);
		const first = store.rows.value[0];
		expect(first.title).toBe('Oak worktop');
		expect(column(store.rows.value, 11)[0]).toBe(680);
		await store.setValue(first.document_id, 9, { type: 'integer', value: 3 });
		// The mock computes the formula at once; the daemon's outputs follow with an invalidation.
		expect(column(store.rows.value, 11)[0]).toBe(1020);
		await store.setValue(first.document_id, 9, null);
		expect(column(store.rows.value, 9)[0]).toBeNull();
		await expect(
			store.setValue(first.document_id, 11, { type: 'currency', value: 1 })
		).rejects.toMatchObject({ field: 'property_id' });
	});

	it('sends filters and sort to the daemon, the title among them, and keeps them for the same table', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		await store.setQuery({
			filters: [
				{
					column: { kind: 'property', property_id: 2 },
					condition: { kind: 'greaterThan', value: { type: 'integer', value: 300 } }
				}
			],
			sort: { column: { kind: 'property', property_id: 3 }, descending: true }
		});
		// Empty ratings sort last either way.
		expect(column(store.rows.value, 3)).toEqual([4.8, 4.6, 4.5, 3.9, null]);
		await store.setQuery({
			filters: [{ column: { kind: 'title' }, condition: { kind: 'contains', text: 'the' } }],
			sort: { column: { kind: 'title' }, descending: false }
		});
		expect(store.rows.value.map((r) => r.title)).toEqual([
			'Klara and the Sun',
			'The Dispossessed',
			'The Left Hand of Darkness'
		]);
		await store.open(1, READING);
		expect(store.query.filters).toHaveLength(1);
		await store.open(1, KITCHEN);
		expect(store.query).toEqual({ filters: [], sort: null });
	});

	it('returns a table to the sort it last had, per folder, after a reload', async () => {
		const data = new Map<string, string>();
		vi.stubGlobal('localStorage', {
			getItem: (k: string) => data.get(k) ?? null,
			setItem: (k: string, v: string) => void data.set(k, v)
		});
		const store = createTablesStore(createMock().send);
		await store.open(1, KITCHEN);
		await store.setQuery({ filters: [], sort: { column: { kind: 'title' }, descending: true } });
		const reloaded = createTablesStore(createMock().send);
		await reloaded.open(1, KITCHEN);
		expect(reloaded.query.sort).toEqual({ column: { kind: 'title' }, descending: true });
		await reloaded.open(1, READING);
		expect(reloaded.query.sort).toBeNull();
		await reloaded.open(1, KITCHEN);
		await reloaded.setQuery({ filters: [], sort: null });
		expect(rememberedSort({ workspace_id: 1, folder_id: KITCHEN })).toBeNull();
		vi.unstubAllGlobals();
	});

	it('shows the daemon’s refusal of a filter its type can’t take', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		await store.setQuery({
			filters: [
				{ column: { kind: 'property', property_id: 2 }, condition: { kind: 'contains', text: '3' } }
			],
			sort: null
		});
		expect(store.rows.error).toMatchObject({ code: 'validationFailed', field: 'filters' });
		await store.setQuery({
			filters: [
				{
					column: { kind: 'title' },
					condition: { kind: 'greaterThan', value: { type: 'text', value: 'M' } }
				}
			],
			sort: null
		});
		expect(store.rows.error).toMatchObject({ code: 'validationFailed', field: 'filters' });
	});

	it('adds, renames, retypes, moves and deletes properties', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		const notes = await store.createProperty(
			{ workspace_id: 1, folder_id: READING },
			'Notes',
			'text'
		);
		const row = store.rows.value[0].document_id;
		await store.setValue(row, notes.id, { type: 'text', value: '42' });
		await store.renameProperty(notes.id, 'Copies');
		const on = { kind: 'property', property_id: notes.id } as const;
		await store.setQuery({
			filters: [{ column: on, condition: { kind: 'contains', text: '4' } }],
			sort: { column: on, descending: false }
		});
		await store.setPropertyType(notes.id, 'integer');
		await store.rows.load(store.rows.key!);
		// The filters and sort on it go with its old type; "42" reads as a whole number.
		expect(store.query).toEqual({ filters: [], sort: null });
		expect(store.properties.value.at(-1)).toMatchObject({ name: 'Copies', data_type: 'integer' });
		expect(column(store.rows.value, notes.id)[0]).toBe(42);
		await store.moveProperty(notes.id, 1);
		expect(store.properties.value[0].id).toBe(notes.id);
		await store.removeProperty(notes.id);
		expect(store.properties.value.map((p) => p.id)).not.toContain(notes.id);
		expect(column(store.rows.value, notes.id).every((v) => v === null)).toBe(true);
	});

	it('shows a folder as a table from its first property, and as a list again when set', async () => {
		const mock = createMock();
		const documents = createDocumentsStore(mock.send);
		const store = createTablesStore(mock.send);
		await documents.folders.load(1);
		const representation = () => documents.folders.value.find((f) => f.id === 1)?.representation;
		expect(representation()).toBe('list');
		const only = await store.createProperty({ workspace_id: 1, folder_id: 1 }, 'Status', 'text');
		await documents.folders.refetch();
		expect(representation()).toBe('table');
		await store.open(1, 1);
		await store.removeProperty(only.id);
		await documents.folders.refetch();
		expect(representation()).toBe('table');
		await documents.setRepresentation(1, 1, 'list');
		expect(representation()).toBe('list');
	});

	it('takes a document created, renamed or moved out into the open table', async () => {
		const mock = createMock();
		const documents = createDocumentsStore(mock.send);
		const store = createTablesStore(mock.send);
		await store.open(1, READING);
		const count = store.rows.value.length;
		const doc = await documents.create(1, 'Middlemarch', '', READING);
		store.took(doc);
		expect(store.rows.value.at(-1)).toMatchObject({ title: 'Middlemarch', cells: [] });
		store.took(await documents.renameDocument(1, doc.id, 'Persuasion'));
		expect(store.rows.value.at(-1)?.title).toBe('Persuasion');
		store.took(await documents.move(doc, null));
		expect(store.rows.value).toHaveLength(count);
	});

	it('opens a row in the panel and keeps it and the table in step', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, KITCHEN);
		const first = store.rows.value[0];
		await store.openRow(1, KITCHEN, first.document_id);
		expect(store.panel.properties.value).toHaveLength(6);
		expect(store.panelDocument.value?.title).toBe('Oak worktop');
		await store.panel.setValue(9, { type: 'integer', value: 3 });
		expect(column([store.panel.row.value!], 11)).toEqual([1020]);
		expect(column(store.rows.value, 11)[0]).toBe(1020);
		await store.setValue(first.document_id, 9, { type: 'integer', value: 4 });
		expect(column([store.panel.row.value!], 9)).toEqual([4]);
		const document = await store.writeRowDocument(
			{ workspace_id: 1, document_id: first.document_id },
			'Measured twice.'
		);
		expect(store.panelDocument.value?.contents).toBe(document.contents);
	});

	it('loses a row once its document leaves the table', async () => {
		const mock = createMock();
		const store = createTablesStore(mock.send);
		const [dune] = await mock.send('listTableRows', {
			workspace_id: 1,
			folder_id: READING,
			filters: []
		});
		await store.openRow(1, READING, dune.document_id);
		expect(store.panel.row.value?.title).toBe('Dune');
		await mock.send('moveDocument', {
			workspace_id: 1,
			document_id: dune.document_id,
			folder_id: null
		});
		await store.panel.row.refetch();
		expect(store.panel.row.value).toBeNull();
	});

	it('records a formula’s failure on its property', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		const stars = store.properties.value.find((p) => p.name === 'Stars');
		// Project Hail Mary has no rating.
		expect(stars?.formula?.error).toMatch(/toFixed/);
		expect(column(store.rows.value, 7)[0]).toBe('4.5 stars');
	});

	it('creates a formula with its update mode, and changes the mode alone', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		const stars = store.properties.value.find((p) => p.name === 'Stars')!;
		expect(stars.formula?.update_mode).toBe('whenChanged');
		const pages = await store.createProperty(
			{ workspace_id: 1, folder_id: READING },
			'Twice',
			'integer',
			{ source: '(row) => row.Pages * 2', update_mode: 'continuously' }
		);
		expect(pages.formula?.update_mode).toBe('continuously');
		// The mode alone keeps the outputs and the error.
		const changed = await store.setPropertyFormula(stars.id, {
			source: stars.formula!.source,
			update_mode: 'continuously'
		});
		expect(changed.formula).toEqual({ ...stars.formula, update_mode: 'continuously' });
		expect(store.properties.value.find((p) => p.id === stars.id)?.formula?.update_mode).toBe(
			'continuously'
		);
		expect(column(store.rows.value, 7)[0]).toBe('4.5 stars');
		// Set by hand again, it has no mode.
		const plain = await store.setPropertyFormula(stars.id, null);
		expect(plain.formula).toBeNull();
	});

	it('picks a choice from its options, filters on one and sorts by their order', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		const status = store.properties.value.find((p) => p.id === STATUS)!;
		expect(optionNames(status)).toEqual(['To read', 'Reading', 'Finished']);
		expect(column(store.rows.value, STATUS)).toEqual([3, 3, 2, 1, 3, null]);
		const klara = store.rows.value[5];
		await store.setValue(klara.document_id, STATUS, { type: 'choice', value: 2 });
		expect(column(store.rows.value, STATUS)[5]).toBe(2);
		// Another column's option, or none, isn't one.
		await expect(
			store.setValue(klara.document_id, STATUS, { type: 'choice', value: 99 })
		).rejects.toMatchObject({ field: 'value' });
		await store.setValue(klara.document_id, STATUS, null);

		await store.setQuery({
			filters: [
				{
					column: { kind: 'property', property_id: STATUS },
					condition: { kind: 'notEquals', value: { type: 'choice', value: 3 } }
				}
			],
			sort: { column: { kind: 'property', property_id: STATUS }, descending: false }
		});
		// By the options' order, not their names; empty last.
		expect(store.rows.value.map((r) => r.title)).toEqual([
			'Project Hail Mary',
			'Piranesi',
			'Klara and the Sun'
		]);
		// Moving an option reorders the sort.
		await store.moveOption(STATUS, 1, null);
		expect(store.rows.value.map((r) => r.title)).toEqual([
			'Piranesi',
			'Project Hail Mary',
			'Klara and the Sun'
		]);
	});

	it('adds, renames, recolors and deletes options, and a deleted one clears its values', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		let status = await store.addOption(STATUS, 'Abandoned');
		expect(optionNames(status)).toEqual(['To read', 'Reading', 'Finished', 'Abandoned']);
		await expect(store.addOption(STATUS, 'reading')).rejects.toMatchObject({ field: 'name' });
		status = await store.renameOption(STATUS, 3, 'Done');
		expect(optionNames(status)).toContain('Done');
		// Values name the option by id, so they keep it.
		expect(column(store.rows.value, STATUS)).toEqual([3, 3, 2, 1, 3, null]);
		status = await store.setOptionColor(STATUS, 3, null);
		expect(status.data_type === 'choice' && status.options[2].color).toBeNull();
		expect(store.optionUsage(STATUS, 3)).toBe(3);
		await store.setQuery({
			filters: [
				{
					column: { kind: 'property', property_id: STATUS },
					condition: { kind: 'equals', value: { type: 'choice', value: 3 } }
				}
			],
			sort: null
		});
		// Filtered, the rows shown can't say how many hold it.
		expect(store.optionUsage(STATUS, 3)).toBeNull();
		status = await store.removeOption(STATUS, 3);
		expect(optionNames(status)).toEqual(['To read', 'Reading', 'Abandoned']);
		// The filter naming it goes, and the values that held it are empty.
		expect(store.query.filters).toEqual([]);
		await store.rows.refetch();
		expect(column(store.rows.value, STATUS)).toEqual([null, null, 2, 1, null, null]);
	});

	it('creates a choice with its options, and retypes text to a choice and back', async () => {
		const store = createTablesStore(createMock().send);
		await store.open(1, READING);
		const format = await store.createProperty(
			{ workspace_id: 1, folder_id: READING },
			'Format',
			'choice',
			null,
			[
				{ name: 'Paperback', color: 'teal' },
				{ name: 'Ebook', color: null }
			]
		);
		expect(optionNames(format)).toEqual(['Paperback', 'Ebook']);
		// Each option has an id of its own.
		const ids = format.data_type === 'choice' ? format.options.map((o) => o.id) : [];
		expect(new Set(ids).size).toBe(2);
		await expect(
			store.createProperty({ workspace_id: 1, folder_id: READING }, 'Mood', 'choice', {
				source: '(row) => 1',
				update_mode: 'whenChanged'
			})
		).rejects.toMatchObject({ field: 'data_type' });

		// Author is text: each different name becomes an option, alphabetically.
		const author = await store.setPropertyType(1, 'choice');
		expect(optionNames(author)).toEqual([
			'Andy Weir',
			'Frank Herbert',
			'Kazuo Ishiguro',
			'Susanna Clarke',
			'Ursula K. Le Guin'
		]);
		await store.rows.refetch();
		const authors = column(store.rows.value, 1);
		expect(authors[1]).toBe(authors[4]);
		const back = await store.setPropertyType(1, 'text');
		expect(back.data_type).toBe('text');
		await store.rows.refetch();
		expect(column(store.rows.value, 1)).toEqual([
			'Frank Herbert',
			'Ursula K. Le Guin',
			'Susanna Clarke',
			'Andy Weir',
			'Ursula K. Le Guin',
			'Kazuo Ishiguro'
		]);
	});
});
