// Databases: a workspace's tables of typed values. The list of a workspace's
// databases, and the one database a page has open with its properties and
// its rows under the page's filters and sort, which the daemon applies.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	Database,
	DatabaseDataType,
	DatabaseFilter,
	DatabaseProperty,
	DatabaseRowEntry,
	DatabaseSort,
	DatabaseValue,
	Document,
	Invalidation,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

export interface DatabaseKey {
	workspace_id: number;
	database_id: number;
}

/** Which rows a database page shows and in what order: the page's own, never stored. */
export interface DatabaseQuery {
	/** Every one must hold. */
	filters: DatabaseFilter[];
	/** The order rows were added in when null. */
	sort: DatabaseSort | null;
}

export interface DatabaseRowsKey extends DatabaseKey, DatabaseQuery {}

/** Every row, in the order they were added. */
export const NO_QUERY: DatabaseQuery = { filters: [], sort: null };

// As the daemon lists them: by title ignoring case, then as created.
const byTitle = (a: Database, b: Database) =>
	a.title.toLowerCase().localeCompare(b.title.toLowerCase()) || a.id - b.id;

const sameQuery = (a: DatabaseQuery, b: DatabaseQuery) =>
	JSON.stringify(a.filters) === JSON.stringify(b.filters) &&
	JSON.stringify(a.sort) === JSON.stringify(b.sort);

/** `query` without the filters and sort on `property_id`. */
export function withoutProperty(query: DatabaseQuery, property_id: number): DatabaseQuery {
	return {
		filters: query.filters.filter((f) => f.property_id !== property_id),
		sort: query.sort?.property_id === property_id ? null : query.sort
	};
}

const inDatabase = (key: Invalidation, { database_id }: DatabaseKey) =>
	key.kind === 'database' && key.id === database_id;

export class DatabasesStore {
	/** The workspace's databases, by title. */
	readonly list: Resource<Database[], number>;
	/** The open database; null once it is deleted. */
	readonly database: Resource<Database | null, DatabaseKey>;
	/** The open database's properties, in their order. */
	readonly properties: Resource<DatabaseProperty[], DatabaseKey>;
	/** The open database's rows that meet the query's filters, in its order. */
	readonly rows: Resource<DatabaseRowEntry[], DatabaseRowsKey>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (workspace_id) =>
				send('listDatabases', { workspace_id }).then((list) => list.toSorted(byTitle)),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'databases' && key.id === workspace_id
		});
		this.database = new Resource({
			fetch: (key) => send('getDatabase', key),
			initial: null,
			matches: (key, db) =>
				inDatabase(key, db) || (key.kind === 'databases' && key.id === db.workspace_id)
		});
		this.properties = new Resource({
			fetch: (key) => send('listDatabaseProperties', key),
			initial: [],
			matches: inDatabase
		});
		this.rows = new Resource({
			fetch: ({ workspace_id, database_id, filters, sort }) =>
				send('listDatabaseRows', { workspace_id, database_id, filters, sort }),
			initial: [],
			matches: (key, rows) => inDatabase(key, rows),
			// A filter or the order changing swaps the rows in place.
			keepsValue: (from, to) => from.database_id === to.database_id
		});
	}

	load(workspace_id: number): Promise<void> {
		return this.list.load(workspace_id);
	}

	/** The filters and sort the open database's rows are listed under. */
	get query(): DatabaseQuery {
		const key = this.rows.key;
		return key ? { filters: key.filters, sort: key.sort } : NO_QUERY;
	}

	/**
	 * Loads a database, its properties and its rows. The same database keeps
	 * the filters and sort it had; another starts with none.
	 */
	async open(workspace_id: number, database_id: number): Promise<void> {
		const current = this.database.key;
		const same = current?.workspace_id === workspace_id && current.database_id === database_id;
		const key = same && current ? current : { workspace_id, database_id };
		const rows = same && this.rows.key ? this.rows.key : { ...key, ...NO_QUERY };
		await Promise.all([this.database.load(key), this.properties.load(key), this.rows.load(rows)]);
	}

	/** Lists the open database's rows under `query`, which the daemon applies. */
	setQuery(query: DatabaseQuery): Promise<void> {
		const key = this.rows.key!;
		if (sameQuery(key, query)) return this.rows.load(key);
		return this.rows.load({
			workspace_id: key.workspace_id,
			database_id: key.database_id,
			...query
		});
	}

	// ---- Databases ------------------------------------------------------------

	async create(workspace_id: number, title: string): Promise<Database> {
		const database = await this.#send('createDatabase', { workspace_id, title });
		this.#put(database);
		return database;
	}

	async rename(workspace_id: number, database_id: number, title: string): Promise<Database> {
		const database = await this.#send('renameDatabase', { workspace_id, database_id, title });
		this.#put(database);
		if (this.database.value?.id === database.id) this.database.set(database);
		return database;
	}

	async remove(workspace_id: number, database_id: number): Promise<Database> {
		const database = await this.#send('deleteDatabase', { workspace_id, database_id });
		if (this.list.key === workspace_id)
			this.list.update((list) => list.filter((d) => d.id !== database_id));
		if (this.database.value?.id === database_id) this.database.set(null);
		return database;
	}

	#put(database: Database) {
		if (this.list.key !== database.workspace_id) return;
		this.list.update((list) =>
			[...list.filter((d) => d.id !== database.id), database].toSorted(byTitle)
		);
	}

	// ---- Properties of the open database --------------------------------------

	get #key(): DatabaseKey {
		return this.database.key!;
	}

	#putProperty(property: DatabaseProperty) {
		this.properties.update((list) => list.map((p) => (p.id === property.id ? property : p)));
	}

	/** Adds a property after the others; a formula when `formula` is given. */
	async createProperty(
		name: string,
		data_type: DatabaseDataType,
		formula: string | null = null
	): Promise<DatabaseProperty> {
		const property = await this.#send('createDatabaseProperty', {
			...this.#key,
			name,
			data_type,
			formula
		});
		this.properties.update((list) => [...list, property]);
		// A formula's first outputs come with the rows, which the reply doesn't carry.
		if (formula !== null) void this.rows.refetch();
		return property;
	}

	async renameProperty(property_id: number, name: string): Promise<DatabaseProperty> {
		const property = await this.#send('renameDatabaseProperty', {
			...this.#key,
			property_id,
			name
		});
		this.#putProperty(property);
		return property;
	}

	/**
	 * Changes a property's type. The daemon keeps the values that read as the
	 * new type and empties the rest, which the reply doesn't carry, so the
	 * rows are listed again, without the filters and sort on the property,
	 * which may not fit its new type.
	 */
	async setPropertyType(
		property_id: number,
		data_type: DatabaseDataType
	): Promise<DatabaseProperty> {
		const property = await this.#send('setDatabasePropertyType', {
			...this.#key,
			property_id,
			data_type
		});
		this.#putProperty(property);
		const query = withoutProperty(this.query, property_id);
		if (sameQuery(query, this.query)) void this.rows.refetch();
		else void this.setQuery(query);
		return property;
	}

	/**
	 * Makes a property a formula, changes its source, or with `formula` null
	 * makes it set by hand again. Its values change with it, so the rows are
	 * listed again.
	 */
	async setPropertyFormula(property_id: number, formula: string | null): Promise<DatabaseProperty> {
		const property = await this.#send('setDatabasePropertyFormula', {
			...this.#key,
			property_id,
			formula
		});
		this.#putProperty(property);
		void this.rows.refetch();
		return property;
	}

	/** Moves a property just before `before_id`, or last when it's null. */
	async moveProperty(property_id: number, before_id: number | null): Promise<void> {
		const properties = await this.#send('moveDatabaseProperty', {
			...this.#key,
			property_id,
			before_id
		});
		this.properties.set(properties);
	}

	/** Deletes a property with its values, and the filters and sort on it. */
	async removeProperty(property_id: number): Promise<DatabaseProperty> {
		const property = await this.#send('deleteDatabaseProperty', { ...this.#key, property_id });
		this.properties.update((list) => list.filter((p) => p.id !== property_id));
		this.rows.update((rows) =>
			rows.map((row) => ({
				...row,
				cells: row.cells.filter((c) => c.property_id !== property_id)
			}))
		);
		const query = withoutProperty(this.query, property_id);
		if (!sameQuery(query, this.query)) void this.setQuery(query);
		return property;
	}

	// ---- Rows of the open database --------------------------------------------

	#putRow(entry: DatabaseRowEntry) {
		this.rows.update((rows) => rows.map((r) => (r.id === entry.id ? entry : r)));
	}

	/** Adds an empty row, last, where an empty row sorts under any order. */
	async createRow(): Promise<DatabaseRowEntry> {
		const entry = await this.#send('createDatabaseRow', this.#key);
		this.rows.update((rows) => [...rows, entry]);
		return entry;
	}

	async removeRow(row_id: number): Promise<void> {
		await this.#send('deleteDatabaseRow', { ...this.#key, row_id });
		this.rows.update((rows) => rows.filter((r) => r.id !== row_id));
	}

	/** Sets a row's value for a property, or empties it when `value` is null. */
	async setValue(
		row_id: number,
		property_id: number,
		value: DatabaseValue | null
	): Promise<DatabaseRowEntry> {
		const entry = await this.#send('setDatabaseValue', {
			...this.#key,
			row_id,
			property_id,
			value
		});
		this.#putRow(entry);
		return entry;
	}

	/** Gives a row a document of its own. */
	async createRowDocument(row_id: number): Promise<Document> {
		const document = await this.#send('createDatabaseRowDocument', { ...this.#key, row_id });
		this.rows.update((rows) =>
			rows.map((r) => (r.id === row_id ? { ...r, document_id: document.id } : r))
		);
		return document;
	}
}

export function createDatabasesStore(send: Send = defaultSend): DatabasesStore {
	return new DatabasesStore(send);
}
