import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { DatabaseRowEntry } from '$lib/protocol/types';
import { createDatabasesStore } from './databases.svelte';

/** Each row's value for `property_id`, as the rows are listed. */
const column = (rows: DatabaseRowEntry[], property_id: number) =>
	rows.map((r) => r.cells.find((c) => c.property_id === property_id)?.value.value ?? null);

// The seed's reading list: 1 Title, 3 Pages, 4 Rating, 6 Read, 8 Stars (a formula of Rating).
// The renovation budget: 11 Quantity, 12 Unit price, 13 Total (a formula of the two).

describe('databases store', () => {
	it('keeps the workspace’s databases by title as they are created, renamed and deleted', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.load(1);
		expect(store.list.value.map((d) => d.title)).toEqual(['Kitchen renovation', 'Reading list']);
		const created = await store.create(1, 'Garden');
		await store.rename(1, created.id, 'Allotment');
		expect(store.list.value.map((d) => d.title)).toEqual([
			'Allotment',
			'Kitchen renovation',
			'Reading list'
		]);
		await expect(store.rename(1, created.id, '')).rejects.toMatchObject({
			code: 'validationFailed'
		});
		await store.remove(1, created.id);
		expect(store.list.value).toHaveLength(2);
	});

	it('opens a database with its properties and rows, and sets values from the reply', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.open(1, 2);
		expect(store.database.value?.title).toBe('Kitchen renovation');
		expect(store.properties.value.map((p) => p.name)).toEqual([
			'Item',
			'Supplier',
			'Quantity',
			'Unit price',
			'Total',
			'Ordered',
			'Delivery'
		]);
		const first = store.rows.value[0];
		expect(column(store.rows.value, 13)[0]).toBe(680);
		await store.setValue(first.id, 11, { type: 'integer', value: 3 });
		// The mock computes the formula at once; the daemon's outputs follow with an invalidation.
		expect(column(store.rows.value, 13)[0]).toBe(1020);
		await store.setValue(first.id, 11, null);
		expect(column(store.rows.value, 11)[0]).toBeNull();
		await expect(
			store.setValue(first.id, 13, { type: 'currency', value: 1 })
		).rejects.toMatchObject({ field: 'property_id' });
	});

	it('sends filters and sort to the daemon and keeps them for the same database', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.open(1, 1);
		await store.setQuery({
			filters: [
				{
					property_id: 3,
					condition: { kind: 'greaterThan', value: { type: 'integer', value: 300 } }
				}
			],
			sort: { property_id: 4, descending: true }
		});
		// Empty ratings sort last either way.
		expect(column(store.rows.value, 4)).toEqual([4.8, 4.6, 4.5, 3.9, null]);
		await store.open(1, 1);
		expect(store.query.filters).toHaveLength(1);
		await store.open(1, 2);
		expect(store.query).toEqual({ filters: [], sort: null });
	});

	it('shows the daemon’s refusal of a filter its type can’t take', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.open(1, 1);
		await store.setQuery({
			filters: [{ property_id: 3, condition: { kind: 'contains', text: '3' } }],
			sort: null
		});
		expect(store.rows.error).toMatchObject({ code: 'validationFailed', field: 'filters' });
	});

	it('adds, renames, retypes, moves and deletes properties', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.open(1, 1);
		const notes = await store.createProperty('Notes', 'text');
		const row = store.rows.value[0].id;
		await store.setValue(row, notes.id, { type: 'text', value: '42' });
		await store.renameProperty(notes.id, 'Copies');
		await store.setQuery({
			filters: [{ property_id: notes.id, condition: { kind: 'contains', text: '4' } }],
			sort: { property_id: notes.id, descending: false }
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

	it('adds and deletes rows, and gives a row a document the document list leaves out', async () => {
		const mock = createMock();
		const store = createDatabasesStore(mock.send);
		await store.open(1, 1);
		const count = store.rows.value.length;
		const row = await store.createRow();
		expect(store.rows.value).toHaveLength(count + 1);
		const document = await store.createRowDocument(row.id);
		expect(store.rows.value.at(-1)?.document_id).toBe(document.id);
		const listed = await mock.send('listDocuments', { workspace_id: 1 });
		expect(listed.map((d) => d.id)).not.toContain(document.id);
		await store.removeRow(row.id);
		expect(store.rows.value).toHaveLength(count);
	});

	it('records a formula’s failure on its property', async () => {
		const store = createDatabasesStore(createMock().send);
		await store.open(1, 1);
		const stars = store.properties.value.find((p) => p.name === 'Stars');
		// Project Hail Mary has no rating.
		expect(stars?.formula?.error).toMatch(/toFixed/);
		expect(column(store.rows.value, 8)[0]).toBe('4.5 stars');
	});
});
