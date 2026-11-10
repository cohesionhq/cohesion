// Databases: a workspace's tables of typed values. The list of a workspace's
// databases, the one database a page has open with its properties and its
// rows under the page's filters and sort, which the daemon applies, and the
// one row open in the panel over the page, with its database's properties
// and its document.

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
import { ProtocolError } from '$lib/protocol/errors';
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

export interface DatabaseRowKey extends DatabaseKey {
	row_id: number;
}

export interface RowDocumentKey {
	workspace_id: number;
	document_id: number;
}

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

/** What `request` returns, or null when it is not found. */
async function orNull<T>(request: Promise<T>): Promise<T | null> {
	try {
		return await request;
	} catch (error) {
		if (error instanceof ProtocolError && error.code === 'notFound') return null;
		throw error;
	}
}

export class DatabasesStore {
	/** The workspace's databases, by title. */
	readonly list: Resource<Database[], number>;
	/** The open database; null once it is deleted. */
	readonly database: Resource<Database | null, DatabaseKey>;
	/** The open database's properties, in their order. */
	readonly properties: Resource<DatabaseProperty[], DatabaseKey>;
	/** The open database's rows that meet the query's filters, in its order. */
	readonly rows: Resource<DatabaseRowEntry[], DatabaseRowsKey>;
	/** The row open in the panel; null once it is deleted. */
	readonly row: Resource<DatabaseRowEntry | null, DatabaseRowKey>;
	/** The open row's database; null once it is deleted. */
	readonly rowDatabase: Resource<Database | null, DatabaseKey>;
	/** The open row's database's properties, in their order. */
	readonly rowProperties: Resource<DatabaseProperty[], DatabaseKey>;
	/** The open row's document, while it has one. */
	readonly rowDocument: Resource<Document | null, RowDocumentKey>;

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
		this.row = new Resource({
			fetch: (key: DatabaseRowKey) => orNull(send('getDatabaseRow', key)),
			initial: null,
			matches: inDatabase,
			onFetched: (row) => void this.#loadRowDocument(row)
		});
		this.rowDatabase = new Resource({
			fetch: (key) => orNull(send('getDatabase', key)),
			initial: null,
			matches: (key, db) =>
				inDatabase(key, db) || (key.kind === 'databases' && key.id === db.workspace_id)
		});
		this.rowProperties = new Resource({
			fetch: (key) => send('listDatabaseProperties', key),
			initial: [],
			matches: inDatabase
		});
		this.rowDocument = new Resource({
			fetch: (key) => orNull(send('getDocument', key)),
			initial: null,
			matches: (key, doc) =>
				(key.kind === 'documents' || key.kind === 'workspace') && key.id === doc.workspace_id
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
		if (this.#isOpenRow(this.#key.database_id, row_id)) this.row.set(null);
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
		this.#took(entry);
		return entry;
	}

	/** A row from a reply, in the open database's rows and the panel, wherever it shows. */
	#took(entry: DatabaseRowEntry) {
		if (this.database.key?.database_id === entry.database_id) this.#putRow(entry);
		if (this.#isOpenRow(entry.database_id, entry.id)) this.row.set(entry);
	}

	#isOpenRow(database_id: number, row_id: number): boolean {
		const key = this.row.key;
		return key?.database_id === database_id && key.row_id === row_id;
	}

	// ---- The row open in the panel ----------------------------------------------

	/** Loads a row for the panel with its database, its properties and its document. */
	async openRow(workspace_id: number, database_id: number, row_id: number): Promise<void> {
		const current = this.row.key;
		const same =
			current?.workspace_id === workspace_id &&
			current.database_id === database_id &&
			current.row_id === row_id;
		const rowKey = same && current ? current : { workspace_id, database_id, row_id };
		const db = this.rowDatabase.key;
		const dbKey =
			db?.workspace_id === workspace_id && db.database_id === database_id
				? db
				: { workspace_id, database_id };
		await Promise.all([
			this.row.load(rowKey),
			this.rowDatabase.load(dbKey),
			this.rowProperties.load(dbKey)
		]);
		// A row already loaded isn't fetched again, so its document is loaded here too.
		await this.#loadRowDocument(this.row.value);
	}

	/** Stops the panel's row refetching once the panel closes. */
	closeRow(): void {
		this.row.release();
		this.rowDatabase.release();
		this.rowProperties.release();
		this.rowDocument.release();
	}

	/** Loads `row`'s document for the panel, or forgets the last one when it has none. */
	async #loadRowDocument(row: DatabaseRowEntry | null): Promise<void> {
		const key = this.row.key;
		if (row === null || row.document_id === null || !key) {
			this.rowDocument.set(null);
			return;
		}
		const current = this.rowDocument.key;
		if (current?.workspace_id === key.workspace_id && current.document_id === row.document_id)
			return this.rowDocument.load(current);
		return this.rowDocument.load({ workspace_id: key.workspace_id, document_id: row.document_id });
	}

	/** Sets the open row's value for a property, or empties it when `value` is null. */
	async setRowValue(property_id: number, value: DatabaseValue | null): Promise<DatabaseRowEntry> {
		const entry = await this.#send('setDatabaseValue', {
			...this.row.key!,
			property_id,
			value
		});
		this.#took(entry);
		return entry;
	}

	/**
	 * Writes the document of row `key`, giving the row one first when it has
	 * none, so a row gains a document only once something is written in it.
	 */
	async writeRowDocument(key: DatabaseRowKey, contents: string): Promise<Document> {
		const { workspace_id, database_id, row_id } = key;
		const open = this.#isOpenRow(database_id, row_id);
		const entry = open ? this.row.value : await this.#send('getDatabaseRow', key);
		let document_id = entry?.document_id ?? null;
		if (document_id === null) {
			const created = await this.#send('createDatabaseRowDocument', key);
			document_id = created.id;
			if (entry) this.#took({ ...entry, document_id });
		}
		const document = await this.#send('editDocument', { workspace_id, document_id, contents });
		if (this.#isOpenRow(database_id, row_id)) {
			if (this.rowDocument.key?.document_id === document.id) this.rowDocument.set(document);
			else await this.rowDocument.load({ workspace_id, document_id: document.id });
		}
		return document;
	}
}

export function createDatabasesStore(send: Send = defaultSend): DatabasesStore {
	return new DatabasesStore(send);
}
