// Tables: document folders with columns, whose documents are their rows. The
// table a page has open, with its properties and its rows under the page's
// filters and sort, which the daemon applies; and one document's values,
// for the row open in the panel over the page and for the document page
// (`DocumentValues`), each with its folder's properties.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	Document,
	DocumentFolderProperty,
	Invalidation,
	FormulaDefinition,
	IntelligenceAgentColor,
	NewPropertyOption,
	PropertyDataType,
	PropertyValue,
	Send,
	TableFilter,
	TableRow,
	TableSort
} from '$lib/protocol/types';
import { ProtocolError } from '$lib/protocol/errors';
import { sameColumn } from '$lib/features/tables/values';
import { Resource } from './resource.svelte';

export interface TableKey {
	workspace_id: number;
	folder_id: number;
}

/** Which rows a table shows and in what order: the page's own, never stored. */
export interface TableQuery {
	/** Every one must hold. */
	filters: TableFilter[];
	/** The order the documents were created in when null. */
	sort: TableSort | null;
}

export interface TableRowsKey extends TableKey, TableQuery {}

export interface TableRowKey extends TableKey {
	document_id: number;
}

export interface RowDocumentKey {
	workspace_id: number;
	document_id: number;
}

/** Every row, in the order the documents were created. */
export const NO_QUERY: TableQuery = { filters: [], sort: null };

const sameQuery = (a: TableQuery, b: TableQuery) =>
	JSON.stringify(a.filters) === JSON.stringify(b.filters) &&
	JSON.stringify(a.sort) === JSON.stringify(b.sort);

/** `query` without the filters that name option `option_id` of property `property_id`. */
export function withoutOption(
	query: TableQuery,
	property_id: number,
	option_id: number
): TableQuery {
	const column = { kind: 'property', property_id } as const;
	return {
		...query,
		filters: query.filters.filter(
			(f) =>
				!sameColumn(f.column, column) ||
				!('value' in f.condition) ||
				f.condition.value.type !== 'choice' ||
				f.condition.value.value !== option_id
		)
	};
}

/** `query` without the filters and sort on property `property_id`. */
export function withoutProperty(query: TableQuery, property_id: number): TableQuery {
	const column = { kind: 'property', property_id } as const;
	return {
		filters: query.filters.filter((f) => !sameColumn(f.column, column)),
		sort: query.sort && sameColumn(query.sort.column, column) ? null : query.sort
	};
}

/** What `request` returns, or null when it is not found. */
async function found<T>(request: Promise<T>): Promise<T | null> {
	try {
		return await request;
	} catch (error) {
		if (error instanceof ProtocolError && error.code === 'notFound') return null;
		throw error;
	}
}

const inTable = (key: Invalidation, { folder_id }: TableKey) =>
	key.kind === 'documentTable' && key.id === folder_id;

/**
 * One document's values as a row of its folder's table, with the folder's
 * properties: what the row's panel and the document page show above its
 * text. The row is null once the document is gone or has left the folder.
 */
export class DocumentValues {
	readonly row: Resource<TableRow | null, TableRowKey>;
	readonly properties: Resource<DocumentFolderProperty[], TableKey>;

	#send: Send;
	#took: (row: TableRow) => void;

	/** `took` gets every row a write here replies with, for whatever else shows it. */
	constructor(send: Send, took: (row: TableRow) => void = () => {}) {
		this.#send = send;
		this.#took = took;
		this.row = new Resource({
			fetch: (key: TableRowKey) => found(send('getTableRow', key)),
			initial: null,
			matches: inTable
		});
		this.properties = new Resource({
			fetch: (key) => found(send('listDocumentFolderProperties', key)).then((list) => list ?? []),
			initial: [],
			matches: inTable
		});
	}

	/** Loads document `document_id` of folder `folder_id` as a row, with the folder's properties. */
	async open(workspace_id: number, folder_id: number, document_id: number): Promise<void> {
		const current = this.row.key;
		const rowKey =
			current?.workspace_id === workspace_id &&
			current.folder_id === folder_id &&
			current.document_id === document_id
				? current
				: { workspace_id, folder_id, document_id };
		const table = this.properties.key;
		const tableKey =
			table?.workspace_id === workspace_id && table.folder_id === folder_id
				? table
				: { workspace_id, folder_id };
		await Promise.all([this.row.load(rowKey), this.properties.load(tableKey)]);
	}

	/** Stops refetching once nothing shows the values. */
	release(): void {
		this.row.release();
		this.properties.release();
	}

	/** Sets the document's value for a property, or empties it when `value` is null. */
	async setValue(property_id: number, value: PropertyValue | null): Promise<TableRow> {
		const row = await this.#send('setDocumentValue', { ...this.row.key!, property_id, value });
		this.row.set(row);
		this.#took(row);
		return row;
	}
}

export class TablesStore {
	/** The open table's properties, in their order. */
	readonly properties: Resource<DocumentFolderProperty[], TableKey>;
	/** The open table's rows that meet the query's filters, in its order. */
	readonly rows: Resource<TableRow[], TableRowsKey>;
	/** The row open in the panel over the page. */
	readonly panel: DocumentValues;
	/** The panel's row's document, which it reads and writes. */
	readonly panelDocument: Resource<Document | null, RowDocumentKey>;
	/** The document page's document, when its folder is a table. */
	readonly page: DocumentValues;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.properties = new Resource({
			fetch: (key) => send('listDocumentFolderProperties', key),
			initial: [],
			matches: inTable
		});
		this.rows = new Resource({
			fetch: ({ workspace_id, folder_id, filters, sort }: TableRowsKey) =>
				send('listTableRows', { workspace_id, folder_id, filters, sort }),
			initial: [],
			matches: inTable,
			// A filter or the order changing swaps the rows in place.
			keepsValue: (from, to) => from.folder_id === to.folder_id
		});
		this.panel = new DocumentValues(send, (row) => this.#tookRow(row));
		this.page = new DocumentValues(send, (row) => this.#tookRow(row));
		this.panelDocument = new Resource({
			fetch: (key) => found(send('getDocument', key)),
			initial: null,
			matches: (key, doc) =>
				(key.kind === 'documents' || key.kind === 'workspace') && key.id === doc.workspace_id
		});
	}

	/** The filters and sort the open table's rows are listed under. */
	get query(): TableQuery {
		const key = this.rows.key;
		return key ? { filters: key.filters, sort: key.sort } : NO_QUERY;
	}

	/**
	 * Loads a table's properties and rows. The same table keeps the filters
	 * and sort it had; another starts with none.
	 */
	async open(workspace_id: number, folder_id: number): Promise<void> {
		const current = this.properties.key;
		const same = current?.workspace_id === workspace_id && current.folder_id === folder_id;
		const key = same && current ? current : { workspace_id, folder_id };
		const rows =
			same && this.rows.key?.folder_id === folder_id ? this.rows.key : { ...key, ...NO_QUERY };
		await Promise.all([this.properties.load(key), this.rows.load(rows)]);
	}

	/** Stops the open table refetching once the page stops showing it. */
	close(): void {
		this.properties.release();
		this.rows.release();
	}

	/** Lists the open table's rows under `query`, which the daemon applies. */
	setQuery(query: TableQuery): Promise<void> {
		const key = this.rows.key!;
		if (sameQuery(key, query)) return this.rows.load(key);
		return this.rows.load({ workspace_id: key.workspace_id, folder_id: key.folder_id, ...query });
	}

	// ---- Properties -------------------------------------------------------------

	/**
	 * Adds a property to folder `key` after its others, a formula when
	 * `formula` is given, a choice with `options`; a folder's first makes it
	 * a table. The open table takes it when it is that folder.
	 */
	async createProperty(
		key: TableKey,
		name: string,
		data_type: PropertyDataType,
		formula: FormulaDefinition | null = null,
		options: NewPropertyOption[] = []
	): Promise<DocumentFolderProperty> {
		const property = await this.#send('createDocumentFolderProperty', {
			...key,
			name,
			data_type,
			formula,
			options
		});
		if (this.properties.key?.folder_id === key.folder_id) {
			this.properties.update((list) => [...list, property]);
			// A formula's first outputs come with the rows, which the reply doesn't carry.
			if (formula !== null) void this.rows.refetch();
		}
		return property;
	}

	get #key(): TableKey {
		return this.properties.key!;
	}

	#putProperty(property: DocumentFolderProperty) {
		this.properties.update((list) => list.map((p) => (p.id === property.id ? property : p)));
	}

	async renameProperty(property_id: number, name: string): Promise<DocumentFolderProperty> {
		const property = await this.#send('renameDocumentFolderProperty', {
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
		data_type: PropertyDataType
	): Promise<DocumentFolderProperty> {
		const property = await this.#send('setDocumentFolderPropertyType', {
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
	 * Makes a property a formula, changes its source or update mode, or with
	 * `formula` null makes it set by hand again. Its values change with it,
	 * so the rows are listed again, unless only the mode changed, which keeps
	 * them.
	 */
	async setPropertyFormula(
		property_id: number,
		formula: FormulaDefinition | null
	): Promise<DocumentFolderProperty> {
		const before = this.properties.value.find((p) => p.id === property_id)?.formula;
		const property = await this.#send('setDocumentFolderPropertyFormula', {
			...this.#key,
			property_id,
			formula
		});
		this.#putProperty(property);
		if (formula === null || before?.source !== formula.source) void this.rows.refetch();
		return property;
	}

	/** Moves a property just before `before_id`, or last when it's null. */
	async moveProperty(property_id: number, before_id: number | null): Promise<void> {
		const properties = await this.#send('moveDocumentFolderProperty', {
			...this.#key,
			property_id,
			before_id
		});
		this.properties.set(properties);
	}

	/**
	 * Deletes a property with its values, and the filters and sort on it;
	 * deleting the last makes the folder a plain one again.
	 */
	async removeProperty(property_id: number): Promise<DocumentFolderProperty> {
		const property = await this.#send('deleteDocumentFolderProperty', {
			...this.#key,
			property_id
		});
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

	// ---- A choice's options ------------------------------------------------------
	// Each reply is the property with its options, which the open table takes.

	/** Adds an option to a choice, last. */
	async addOption(property_id: number, name: string): Promise<DocumentFolderProperty> {
		const property = await this.#send('createPropertyOption', {
			...this.#key,
			property_id,
			name,
			color: null
		});
		this.#putProperty(property);
		return property;
	}

	/** Renames an option; the values holding it name it by id, so they show the new name. */
	async renameOption(
		property_id: number,
		option_id: number,
		name: string
	): Promise<DocumentFolderProperty> {
		const property = await this.#send('renamePropertyOption', {
			...this.#key,
			property_id,
			option_id,
			name
		});
		this.#putProperty(property);
		return property;
	}

	/** Changes an option's color, or takes it away when null. */
	async setOptionColor(
		property_id: number,
		option_id: number,
		color: IntelligenceAgentColor | null
	): Promise<DocumentFolderProperty> {
		const property = await this.#send('setPropertyOptionColor', {
			...this.#key,
			property_id,
			option_id,
			color
		});
		this.#putProperty(property);
		return property;
	}

	/**
	 * Moves an option just before `before_id`, or last when it's null. A
	 * sort on the choice follows its options' order, so the rows are listed
	 * again when they are sorted on it.
	 */
	async moveOption(
		property_id: number,
		option_id: number,
		before_id: number | null
	): Promise<DocumentFolderProperty> {
		const property = await this.#send('movePropertyOption', {
			...this.#key,
			property_id,
			option_id,
			before_id
		});
		this.#putProperty(property);
		const sort = this.query.sort;
		if (sort && sameColumn(sort.column, { kind: 'property', property_id }))
			void this.rows.refetch();
		return property;
	}

	/**
	 * Deletes an option: the values holding it are emptied, and the filters
	 * naming it go.
	 */
	async removeOption(property_id: number, option_id: number): Promise<DocumentFolderProperty> {
		const property = await this.#send('deletePropertyOption', {
			...this.#key,
			property_id,
			option_id
		});
		this.#putProperty(property);
		const held = (c: TableRow['cells'][number]) =>
			c.property_id === property_id && c.value.type === 'choice' && c.value.value === option_id;
		this.rows.update((rows) =>
			rows.map((row) => ({ ...row, cells: row.cells.filter((c) => !held(c)) }))
		);
		const query = withoutOption(this.query, property_id, option_id);
		if (!sameQuery(query, this.query)) void this.setQuery(query);
		return property;
	}

	/**
	 * How many of the open table's documents hold option `option_id`, or null
	 * when the rows listed are filtered, so some may be missing.
	 */
	optionUsage(property_id: number, option_id: number): number | null {
		if (this.query.filters.length > 0) return null;
		return this.rows.value.filter((row) =>
			row.cells.some(
				(c) =>
					c.property_id === property_id && c.value.type === 'choice' && c.value.value === option_id
			)
		).length;
	}

	// ---- Rows of the open table ---------------------------------------------------

	/** Sets a row's value for a property, or empties it when `value` is null. */
	async setValue(
		document_id: number,
		property_id: number,
		value: PropertyValue | null
	): Promise<TableRow> {
		const row = await this.#send('setDocumentValue', {
			...this.#key,
			document_id,
			property_id,
			value
		});
		this.#tookRow(row);
		return row;
	}

	/** A row from a reply, wherever it shows: the open table, the panel and the document page. */
	#tookRow(row: TableRow) {
		if (this.rows.key?.folder_id === row.folder_id)
			this.rows.update((rows) => rows.map((r) => (r.document_id === row.document_id ? row : r)));
		for (const values of [this.panel, this.page])
			if (values.row.value?.document_id === row.document_id) values.row.set(row);
	}

	/**
	 * Takes a document a reply brought into the open table's rows: a new one
	 * in its folder last, where a row with no values sorts under any order; a
	 * renamed one's title; one that left the folder out.
	 */
	took(doc: Document): void {
		const key = this.rows.key;
		if (!key || key.workspace_id !== doc.workspace_id) return;
		const here = doc.folder_id === key.folder_id && !doc.deleted;
		this.rows.update((rows) => {
			const at = rows.findIndex((r) => r.document_id === doc.id);
			if (!here) return at < 0 ? rows : rows.toSpliced(at, 1);
			if (at < 0)
				return [
					...rows,
					{ document_id: doc.id, folder_id: key.folder_id, title: doc.title, cells: [] }
				];
			return rows.map((r) => (r.document_id === doc.id ? { ...r, title: doc.title } : r));
		});
		for (const values of [this.panel, this.page]) {
			const row = values.row.value;
			if (row?.document_id === doc.id && here) values.row.set({ ...row, title: doc.title });
		}
	}

	// ---- The row open in the panel --------------------------------------------------

	/** Loads a row for the panel with its folder's properties and its document. */
	async openRow(workspace_id: number, folder_id: number, document_id: number): Promise<void> {
		const current = this.panelDocument.key;
		const docKey =
			current?.workspace_id === workspace_id && current.document_id === document_id
				? current
				: { workspace_id, document_id };
		await Promise.all([
			this.panel.open(workspace_id, folder_id, document_id),
			this.panelDocument.load(docKey)
		]);
	}

	/** Stops the panel's row refetching once the panel closes. */
	closeRow(): void {
		this.panel.release();
		this.panelDocument.release();
	}

	/** Writes the panel's row's document. */
	async writeRowDocument(key: RowDocumentKey, contents: string): Promise<Document> {
		const document = await this.#send('editDocument', { ...key, contents });
		if (this.panelDocument.key?.document_id === document.id) this.panelDocument.set(document);
		return document;
	}
}

export function createTablesStore(send: Send = defaultSend): TablesStore {
	return new TablesStore(send);
}
