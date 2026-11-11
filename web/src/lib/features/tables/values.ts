// How a table's values read and are typed on the page: a cell's text, an
// editor's text read back as a value, the columns a filter or an order can
// name (the title, then each property), the conditions a filter on each type
// offers and how a filter or an order is worded. A choice names one of its
// property's options by id, and reads as that option's name. The daemon
// checks every value and applies every filter and order; these only shape
// what is sent.

import { localDay, shortDay } from '$lib/format/dates';
import type {
	DocumentFolderProperty,
	PropertyDataType,
	PropertyOption,
	PropertyValue,
	TableColumn,
	TableCondition,
	TableFilter,
	TableRow
} from '$lib/protocol/types';

/** A row's value for a property, or null when it holds none. */
export function cellValue(row: TableRow, property_id: number): PropertyValue | null {
	return row.cells.find((c) => c.property_id === property_id)?.value ?? null;
}

/** A property's options: a choice's, in their order; none for any other type. */
export function optionsOf(property: DocumentFolderProperty): PropertyOption[] {
	return property.data_type === 'choice' ? property.options : [];
}

/** The option `option_id` among `options`, or null once it is gone. */
export function optionOf(options: PropertyOption[], option_id: number): PropertyOption | null {
	return options.find((o) => o.id === option_id) ?? null;
}

/**
 * Whether `row` holds a value that is set by hand, which moving its document
 * out of the table loses; a formula's outputs come back wherever it goes.
 */
export function holdsValues(row: TableRow, properties: DocumentFolderProperty[]): boolean {
	const byHand = new Set(properties.filter((p) => p.formula === null).map((p) => p.id));
	return row.cells.some((c) => byHand.has(c.property_id));
}

/** A column a filter or an order names, as the page shows it: the title, or a property. */
export interface ColumnInfo {
	column: TableColumn;
	name: string;
	data_type: PropertyDataType;
	/** A choice's options, which its filters pick from; none for any other type. */
	options: PropertyOption[];
}

/** The documents' titles, the first column of every table: text, and never empty. */
export const TITLE_COLUMN: ColumnInfo = {
	column: { kind: 'title' },
	name: 'Title',
	data_type: 'text',
	options: []
};

/** A property as a column. */
export function propertyColumn(property: DocumentFolderProperty): ColumnInfo {
	return {
		column: { kind: 'property', property_id: property.id },
		name: property.name,
		data_type: property.data_type,
		options: optionsOf(property)
	};
}

/** Every column of a table: the title, then its properties in their order. */
export function tableColumns(properties: DocumentFolderProperty[]): ColumnInfo[] {
	return [TITLE_COLUMN, ...properties.map(propertyColumn)];
}

/** Whether two columns are the same one. */
export function sameColumn(a: TableColumn, b: TableColumn): boolean {
	return a.kind === 'title'
		? b.kind === 'title'
		: b.kind === 'property' && a.property_id === b.property_id;
}

/** A column as a string, for a select's value: `title`, or the property's id. */
export function columnKey(column: TableColumn): string {
	return column.kind === 'title' ? 'title' : String(column.property_id);
}

/** The column `column` names among `properties`, or undefined once its property is gone. */
export function columnInfo(
	column: TableColumn,
	properties: DocumentFolderProperty[]
): ColumnInfo | undefined {
	return tableColumns(properties).find((c) => sameColumn(c.column, column));
}

/** Whether a type's values are numbers, which read right-aligned. */
export function isNumeric(type: PropertyDataType): boolean {
	return type === 'integer' || type === 'decimal' || type === 'currency';
}

/**
 * A value as a cell shows it, in the reader's locale: numbers grouped, an
 * amount to two places with no currency sign (a table names no
 * currency), a date as "Oct 3" (with its year outside this one), a checkbox
 * as "Checked" or "Not checked" (the table draws a box instead), a choice as
 * its option's name among `options` (the table draws a pill instead).
 */
export function formatValue(
	value: PropertyValue,
	options: PropertyOption[] = [],
	today = localDay(),
	locale?: string
): string {
	switch (value.type) {
		case 'text':
			return value.value;
		case 'integer':
			return value.value.toLocaleString(locale);
		case 'decimal':
			return value.value.toLocaleString(locale, { maximumFractionDigits: 10 });
		case 'currency':
			return value.value.toLocaleString(locale, {
				minimumFractionDigits: 2,
				maximumFractionDigits: 2
			});
		case 'checkbox':
			return value.value ? 'Checked' : 'Not checked';
		case 'date':
			return shortDay(value.value, today, locale);
		case 'choice':
			return optionOf(options, value.value)?.name ?? '';
	}
}

/** A value as an editor starts with it: plain digits with a point, a date as `YYYY-MM-DD`. */
export function inputText(value: PropertyValue | null): string {
	if (value === null) return '';
	return String(value.value);
}

/** What an editor's text reads as: a value, none when it's blank, or why it can't be one. */
export type ParsedInput = { value: PropertyValue | null } | { error: string };

/** Reads an editor's text as a value of `type`; blank text is no value. */
export function parseInput(type: PropertyDataType, text: string): ParsedInput {
	const trimmed = text.trim();
	if (type === 'text') return { value: text === '' ? null : { type, value: text } };
	if (trimmed === '') return { value: null };
	switch (type) {
		case 'integer':
			return /^[+-]?\d+$/.test(trimmed) && Number.isSafeInteger(Number(trimmed))
				? { value: { type, value: Number(trimmed) } }
				: { error: 'Enter a whole number.' };
		case 'decimal':
		case 'currency': {
			const number = Number(trimmed);
			return Number.isFinite(number)
				? { value: { type, value: number } }
				: { error: 'Enter a number.' };
		}
		case 'date':
			return /^\d{4}-\d{2}-\d{2}$/.test(trimmed)
				? { value: { type, value: trimmed } }
				: { error: 'Enter a date.' };
		case 'checkbox':
			return trimmed === 'true' || trimmed === 'false'
				? { value: { type, value: trimmed === 'true' } }
				: { error: 'Choose checked or not.' };
		case 'choice':
			// A choice's text is the id of the option picked from a list.
			return /^\d+$/.test(trimmed)
				? { value: { type, value: Number(trimmed) } }
				: { error: 'Choose an option.' };
	}
}

/** One way a filter can test a column, as its menu offers it. */
export interface ConditionChoice {
	id: string;
	/** Sentence case for the menu ("Is greater than"); lowercased after the column's name in a pill. */
	label: string;
	/** What the user gives: a value of the column's type (a choice's picked from its options), text to look for, or nothing. */
	takes: 'value' | 'text' | 'nothing';
	/** The condition, given the value (`takes: 'value'`) or text (`takes: 'text'`). */
	condition: (value: PropertyValue | null, text: string) => TableCondition;
}

const valueChoice = (
	id: string,
	label: string,
	kind: 'equals' | 'notEquals' | 'greaterThan' | 'greaterOrEqual' | 'lessThan' | 'lessOrEqual'
): ConditionChoice => ({
	id,
	label,
	takes: 'value',
	condition: (value) => ({ kind, value: value! })
});

const EMPTY_CHOICES: ConditionChoice[] = [
	{ id: 'isEmpty', label: 'Is empty', takes: 'nothing', condition: () => ({ kind: 'isEmpty' }) },
	{
		id: 'isNotEmpty',
		label: 'Isn’t empty',
		takes: 'nothing',
		condition: () => ({ kind: 'isNotEmpty' })
	}
];

/**
 * The conditions a filter on a column of `type` offers, as the daemon allows
 * them; a title is never empty, so it offers no emptiness.
 */
export function conditionChoices(type: PropertyDataType, title = false): ConditionChoice[] {
	if (title)
		return conditionChoices(type).filter((c) => c.id !== 'isEmpty' && c.id !== 'isNotEmpty');
	const is = [
		valueChoice('equals', 'Is', 'equals'),
		valueChoice('notEquals', 'Isn’t', 'notEquals')
	];
	switch (type) {
		case 'text':
			return [
				{
					id: 'contains',
					label: 'Contains',
					takes: 'text',
					condition: (_, text) => ({ kind: 'contains', text })
				},
				{
					id: 'notContains',
					label: 'Doesn’t contain',
					takes: 'text',
					condition: (_, text) => ({ kind: 'notContains', text })
				},
				...is,
				...EMPTY_CHOICES
			];
		case 'integer':
		case 'decimal':
		case 'currency':
			return [
				...is,
				valueChoice('greaterThan', 'Is greater than', 'greaterThan'),
				valueChoice('greaterOrEqual', 'Is at least', 'greaterOrEqual'),
				valueChoice('lessThan', 'Is less than', 'lessThan'),
				valueChoice('lessOrEqual', 'Is at most', 'lessOrEqual'),
				...EMPTY_CHOICES
			];
		case 'date':
			return [
				...is,
				valueChoice('greaterThan', 'Is after', 'greaterThan'),
				valueChoice('greaterOrEqual', 'Is on or after', 'greaterOrEqual'),
				valueChoice('lessThan', 'Is before', 'lessThan'),
				valueChoice('lessOrEqual', 'Is on or before', 'lessOrEqual'),
				...EMPTY_CHOICES
			];
		case 'choice':
			return [...is, ...EMPTY_CHOICES];
		case 'checkbox':
			// An empty checkbox shows unchecked, so "isn't checked" keeps empty ones too.
			return [
				{
					id: 'checked',
					label: 'Is checked',
					takes: 'nothing',
					condition: () => ({ kind: 'equals', value: { type: 'checkbox', value: true } })
				},
				{
					id: 'unchecked',
					label: 'Isn’t checked',
					takes: 'nothing',
					condition: () => ({ kind: 'notEquals', value: { type: 'checkbox', value: true } })
				}
			];
	}
}

/** The choice `condition` was made from, for a column of `type`. */
function choiceOf(type: PropertyDataType, condition: TableCondition): ConditionChoice | null {
	if (type === 'checkbox' && (condition.kind === 'equals' || condition.kind === 'notEquals')) {
		const checked = condition.value.type === 'checkbox' && condition.value.value;
		const id = (condition.kind === 'equals') === checked ? 'checked' : 'unchecked';
		return conditionChoices(type).find((c) => c.id === id) ?? null;
	}
	return conditionChoices(type).find((c) => c.id === condition.kind) ?? null;
}

/** A filter in words, for its pill: "Pages is greater than 300", "Author contains “Le Guin”". */
export function filterText(filter: TableFilter, column: ColumnInfo): string {
	const { condition } = filter;
	const label = choiceOf(column.data_type, condition)?.label.toLowerCase() ?? condition.kind;
	if ('text' in condition) return `${column.name} ${label} “${condition.text}”`;
	if ('value' in condition && column.data_type !== 'checkbox') {
		const value =
			condition.value.type === 'text' || condition.value.type === 'choice'
				? `“${formatValue(condition.value, column.options)}”`
				: formatValue(condition.value);
		return `${column.name} ${label} ${value}`;
	}
	return `${column.name} ${label}`;
}

/** How a type orders, ascending then descending: "A to Z", "Z to A". */
export function sortDirections(type: PropertyDataType): [string, string] {
	switch (type) {
		case 'text':
			return ['A to Z', 'Z to A'];
		case 'date':
			return ['Earliest first', 'Latest first'];
		case 'checkbox':
			return ['Unchecked first', 'Checked first'];
		case 'choice':
			return ['First option first', 'Last option first'];
		default:
			return ['Lowest first', 'Highest first'];
	}
}
