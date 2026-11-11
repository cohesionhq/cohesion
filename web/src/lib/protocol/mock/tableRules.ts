// A table's rules as the daemon has them, in JS: what a value may be, how
// formulas are computed, and how rows filter and sort. `tableRules.test.ts`
// runs the cases the daemon's own tests run
// (`cohesion-db/src/commands/document_value/table_cases.json`), so the two
// can't drift apart unseen.

import { ProtocolError } from '../errors';
import { compileMockFormula } from '../mockFormula';
import type {
	Document,
	DocumentFolderProperty,
	PropertyDataType,
	PropertyOption,
	PropertyValue,
	TableColumn,
	TableCondition,
	TableFilter,
	TableRow,
	TableSort
} from '../types';
import type { MockState } from './state';

/** Whether `text` is a real day written `YYYY-MM-DD`. */
export function isDay(text: string): boolean {
	if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
	const [y, m, d] = text.split('-').map(Number);
	const date = new Date(Date.UTC(y, m - 1, d));
	return date.getUTCFullYear() === y && date.getUTCMonth() === m - 1 && date.getUTCDate() === d;
}

/** `value` as the daemon stores it, or null for an empty one; throws the daemon's message for a bad one. */
export function checkedValue(value: PropertyValue): PropertyValue | null {
	switch (value.type) {
		case 'text':
			return value.value === '' ? null : value;
		case 'integer':
			if (!Number.isInteger(value.value)) throw new Error('A whole number has no fraction.');
			return value;
		case 'decimal':
		case 'currency':
			if (!Number.isFinite(value.value)) throw new Error('A number must be finite.');
			return value;
		case 'date':
			if (!isDay(value.value)) throw new Error('A date must be a real day, as YYYY-MM-DD.');
			return value;
		case 'checkbox':
		case 'choice':
			return value;
	}
}

/** A value's text, as a property changing type reads it: a choice's is its option's name. */
export function valueText(property: DocumentFolderProperty, value: PropertyValue): string {
	if (value.type === 'choice') return optionOfProperty(property, value.value)?.name ?? '';
	return String(value.value);
}

/** A choice's option `option_id`, or null when it has none by that id. */
export function optionOfProperty(property: DocumentFolderProperty, option_id: number) {
	return property.data_type === 'choice'
		? (property.options.find((o) => o.id === option_id) ?? null)
		: null;
}

/** A property of `data_type` from the fields every type has: a choice with `options`, any other type with none. */
export function typedProperty(
	base: Omit<DocumentFolderProperty, 'data_type' | 'options'>,
	data_type: PropertyDataType,
	options: PropertyOption[] = []
): DocumentFolderProperty {
	const { id, folder_id, name, sort_order, formula } = base;
	const fields = { id, folder_id, name, sort_order, formula };
	return data_type === 'choice' ? { ...fields, data_type, options } : { ...fields, data_type };
}

/** The next id for an option, unique across every property's. */
export function nextOptionId(state: MockState): number {
	return (
		state.folderProperties
			.flatMap((p) => (p.data_type === 'choice' ? p.options : []))
			.reduce((max, o) => Math.max(max, o.id), 0) + 1
	);
}

/** `text` read as `data_type`, as `PropertyValue::parse` reads it, or null when it isn't one. */
export function parseValue(data_type: PropertyDataType, text: string): PropertyValue | null {
	const number = text.trim() === text && text !== '' ? Number(text) : NaN;
	let value: PropertyValue | null;
	switch (data_type) {
		case 'text':
		case 'date':
			value = { type: data_type, value: text };
			break;
		case 'integer':
			value = /^[+-]?\d+$/.test(text) ? { type: 'integer', value: Number(text) } : null;
			break;
		case 'decimal':
		case 'currency':
			value = Number.isFinite(number) ? { type: data_type, value: number } : null;
			break;
		case 'checkbox':
			value =
				text === 'true' || text === 'false' ? { type: 'checkbox', value: text === 'true' } : null;
			break;
		case 'choice':
			// No text is a choice by itself: becoming one makes options of the texts.
			value = null;
			break;
	}
	try {
		return value && checkedValue(value);
	} catch {
		return null;
	}
}

/** What a value compares as: text ignoring case, a checkbox as 1 or 0, a date by its text. */
export function comparedValue(value: PropertyValue): string | number {
	if (value.type === 'text') return value.value.toLowerCase();
	if (value.type === 'checkbox') return value.value ? 1 : 0;
	return value.value;
}

export function folderProperties(state: MockState, folder_id: number): DocumentFolderProperty[] {
	return state.folderProperties
		.filter((p) => p.folder_id === folder_id)
		.sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
}

export function documentValue(
	state: MockState,
	document_id: number,
	property_id: number
): PropertyValue | null {
	return (
		state.documentValues.find((v) => v.document_id === document_id && v.property_id === property_id)
			?.value ?? null
	);
}

export function storeValue(
	state: MockState,
	document_id: number,
	property_id: number,
	value: PropertyValue | null
) {
	state.documentValues = state.documentValues.filter(
		(v) => !(v.document_id === document_id && v.property_id === property_id)
	);
	if (value !== null) state.documentValues.push({ document_id, property_id, value });
}

/** `output`, a formula's, as a value of `data_type`; throws when it is of another type. */
export function formulaOutput(data_type: PropertyDataType, output: unknown): PropertyValue | null {
	if (output === null || output === undefined) return null;
	const wrong = () => {
		throw new Error(`The formula returned ${JSON.stringify(output)}, which isn’t of its type.`);
	};
	switch (data_type) {
		case 'text':
		case 'date':
			if (typeof output !== 'string') wrong();
			return checkedValue({ type: data_type, value: output as string });
		case 'integer':
		case 'decimal':
		case 'currency':
			if (typeof output !== 'number') wrong();
			return checkedValue({ type: data_type, value: output as number });
		case 'checkbox':
			if (typeof output !== 'boolean') wrong();
			return { type: 'checkbox', value: output as boolean };
		case 'choice':
			// A formula is never a choice.
			return wrong();
	}
}

/** The documents in a folder, deleted ones too, which keep their values; ordered as created. */
export function folderDocuments(state: MockState, folder_id: number): Document[] {
	return state.documents
		.filter((d) => d.folder_id === folder_id && d.document_type === 'normal')
		.sort((a, b) => a.id - b.id);
}

/**
 * Computes every formula of a folder for every document in it, at once
 * rather than a moment after the write as the daemon does, so the reply to
 * a write carries its outputs.
 */
export function computeFormulas(state: MockState, folder_id: number) {
	const properties = folderProperties(state, folder_id);
	// Of two inputs with one name, the one created first.
	const inputs = properties.filter((p) => p.formula === null).sort((a, b) => a.id - b.id);
	const docs = folderDocuments(state, folder_id);
	for (const property of properties) {
		if (property.formula === null) continue;
		const run = compileMockFormula(property.formula.source);
		let error: string | null = null;
		for (const doc of docs) {
			const input: Record<string, unknown> = {};
			for (const p of inputs) {
				if (p.name in input) continue;
				const value = documentValue(state, doc.id, p.id);
				// A formula reads a choice as its option's name.
				input[p.name] =
					value?.type === 'choice'
						? (optionOfProperty(p, value.value)?.name ?? null)
						: (value?.value ?? null);
			}
			let value: PropertyValue | null = null;
			try {
				if (!run) throw new Error('The mock runs only simple formulas (see mockFormula.ts).');
				value = formulaOutput(property.data_type, run(input));
			} catch (e) {
				error ??= (e instanceof Error ? e.message : String(e)).slice(0, 300);
			}
			storeValue(state, doc.id, property.id, value);
		}
		property.formula = { ...property.formula, error };
	}
}

/** `doc` as a row of its folder, with its cells in the folder's property order. */
export function tableRow(state: MockState, doc: Document): TableRow {
	return {
		document_id: doc.id,
		folder_id: doc.folder_id!,
		title: doc.title,
		cells: folderProperties(state, doc.folder_id!).flatMap((p) => {
			const value = documentValue(state, doc.id, p.id);
			return value === null ? [] : [{ property_id: p.id, value: { ...value } }];
		})
	};
}

/** A refusal of a filter, as the daemon words it. */
export function badFilter(message: string): never {
	throw new ProtocolError('validationFailed', message, 'filters');
}

/** The test a column's value must pass; throws for a filter its type can't take. */
export function columnTest(
	data_type: PropertyDataType,
	title: boolean,
	condition: TableCondition
): (value: PropertyValue | null) => boolean {
	const typed = (value: PropertyValue) => {
		if (value.type !== data_type) badFilter('A filter’s value must be of its column’s type.');
		try {
			checkedValue(value);
		} catch (e) {
			badFilter((e as Error).message);
		}
		return comparedValue(value);
	};
	const ordered = (
		value: PropertyValue,
		test: (a: string | number, b: string | number) => boolean
	) => {
		if (title || !['integer', 'decimal', 'currency', 'date'].includes(data_type))
			badFilter('Only numbers, amounts and dates compare as greater or less.');
		const bound = typed(value);
		return (v: PropertyValue | null) => v !== null && test(comparedValue(v), bound);
	};
	const contains = (text: string) => {
		if (data_type !== 'text') badFilter('Only text can contain text.');
		const lower = text.toLowerCase();
		return (v: PropertyValue | null) => v !== null && String(v.value).toLowerCase().includes(lower);
	};
	const equal = (value: PropertyValue) => {
		const bound = typed(value);
		return (v: PropertyValue | null) => v !== null && comparedValue(v) === bound;
	};
	const not = (test: (v: PropertyValue | null) => boolean) => (v: PropertyValue | null) => !test(v);
	switch (condition.kind) {
		case 'isEmpty':
			return (v) => v === null;
		case 'isNotEmpty':
			return (v) => v !== null;
		case 'equals':
			return equal(condition.value);
		case 'notEquals':
			return not(equal(condition.value));
		case 'contains':
			return contains(condition.text);
		case 'notContains':
			return not(contains(condition.text));
		case 'greaterThan':
			return ordered(condition.value, (a, b) => a > b);
		case 'greaterOrEqual':
			return ordered(condition.value, (a, b) => a >= b);
		case 'lessThan':
			return ordered(condition.value, (a, b) => a < b);
		case 'lessOrEqual':
			return ordered(condition.value, (a, b) => a <= b);
	}
}

/** A folder's live documents meeting every filter, in `sort`'s order (empty values last), ties as created. */
export function listTableRows(
	state: MockState,
	folder_id: number,
	filters: TableFilter[],
	sort: TableSort | null | undefined
): Document[] {
	const properties = folderProperties(state, folder_id);
	const propertyOf = (id: number) =>
		properties.find((p) => p.id === id) ??
		(() => {
			throw new ProtocolError('notFound', 'The requested document folder property wasn’t found.');
		})();
	/** What a sort on `column` compares: a choice by where its option sits among its property's. */
	const sortKey = (column: TableColumn, value: PropertyValue) => {
		if (column.kind !== 'property' || value.type !== 'choice') return comparedValue(value);
		const property = propertyOf(column.property_id);
		return property.data_type === 'choice'
			? property.options.findIndex((o) => o.id === value.value)
			: comparedValue(value);
	};
	/** What `doc` holds in `column`: its title as text, or its value. */
	const valueOf = (column: TableColumn) => {
		if (column.kind === 'title')
			return (doc: Document): PropertyValue => ({ type: 'text', value: doc.title });
		const { id } = propertyOf(column.property_id);
		return (doc: Document) => documentValue(state, doc.id, id);
	};
	const tests = filters.map((f) => {
		const title = f.column.kind === 'title';
		const type = f.column.kind === 'title' ? 'text' : propertyOf(f.column.property_id).data_type;
		return { read: valueOf(f.column), test: columnTest(type, title, f.condition) };
	});
	const rows = folderDocuments(state, folder_id)
		.filter((d) => !d.deleted)
		.filter((d) => tests.every(({ read, test }) => test(read(d))));
	if (!sort) return rows;
	const read = valueOf(sort.column);
	return rows.sort((a, b) => {
		const va = read(a);
		const vb = read(b);
		if (va === null || vb === null)
			return (va === null ? 1 : 0) - (vb === null ? 1 : 0) || a.id - b.id;
		const [x, y] = [sortKey(sort.column, va), sortKey(sort.column, vb)];
		const order = x < y ? -1 : x > y ? 1 : 0;
		return (sort.descending ? -order : order) || a.id - b.id;
	});
}
