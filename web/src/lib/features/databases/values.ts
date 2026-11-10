// How a database's values read and are typed on the page: a cell's text, an
// editor's text read back as a value, the conditions a filter on each type
// offers and how a filter or an order is worded. The daemon checks every
// value and applies every filter and order; these only shape what is sent.

import { localDay, shortDay } from '$lib/format/dates';
import type {
	DatabaseCondition,
	DatabaseDataType,
	DatabaseFilter,
	DatabaseProperty,
	DatabaseRowEntry,
	DatabaseValue
} from '$lib/protocol/types';

/** A row's value for a property, or null when it holds none. */
export function cellValue(row: DatabaseRowEntry, property_id: number): DatabaseValue | null {
	return row.cells.find((c) => c.property_id === property_id)?.value ?? null;
}

/** What names a row: its value for the database's first text property, else "Untitled", as the daemon labels it. */
export function rowLabel(row: DatabaseRowEntry, properties: DatabaseProperty[]): string {
	const first = properties.find((p) => p.data_type === 'text');
	const value = first ? cellValue(row, first.id) : null;
	return value?.type === 'text' && value.value.trim() !== '' ? value.value : 'Untitled';
}

/** Whether a type's values are numbers, which read right-aligned. */
export function isNumeric(type: DatabaseDataType): boolean {
	return type === 'integer' || type === 'decimal' || type === 'currency';
}

/**
 * A value as a cell shows it, in the reader's locale: numbers grouped, an
 * amount to two places with no currency sign (the database names no
 * currency), a date as "Oct 3" (with its year outside this one), a checkbox
 * as "Checked" or "Not checked" (the table draws a box instead).
 */
export function formatValue(value: DatabaseValue, today = localDay(), locale?: string): string {
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
	}
}

/** A value as an editor starts with it: plain digits with a point, a date as `YYYY-MM-DD`. */
export function inputText(value: DatabaseValue | null): string {
	if (value === null) return '';
	return String(value.value);
}

/** What an editor's text reads as: a value, none when it's blank, or why it can't be one. */
export type ParsedInput = { value: DatabaseValue | null } | { error: string };

/** Reads an editor's text as a value of `type`; blank text is no value. */
export function parseInput(type: DatabaseDataType, text: string): ParsedInput {
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
	}
}

/** One way a filter can test a property, as its menu offers it. */
export interface ConditionChoice {
	id: string;
	/** Sentence case for the menu ("Is greater than"); lowercased after the property's name in a pill. */
	label: string;
	/** What the user gives: a value of the property's type, text to look for, or nothing. */
	takes: 'value' | 'text' | 'nothing';
	/** The condition, given the value (`takes: 'value'`) or text (`takes: 'text'`). */
	condition: (value: DatabaseValue | null, text: string) => DatabaseCondition;
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

/** The conditions a filter on a property of `type` offers, as the daemon allows them. */
export function conditionChoices(type: DatabaseDataType): ConditionChoice[] {
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

/** The choice `condition` was made from, for a property of `type`. */
function choiceOf(type: DatabaseDataType, condition: DatabaseCondition): ConditionChoice | null {
	if (type === 'checkbox' && (condition.kind === 'equals' || condition.kind === 'notEquals')) {
		const checked = condition.value.type === 'checkbox' && condition.value.value;
		const id = (condition.kind === 'equals') === checked ? 'checked' : 'unchecked';
		return conditionChoices(type).find((c) => c.id === id) ?? null;
	}
	return conditionChoices(type).find((c) => c.id === condition.kind) ?? null;
}

/** A filter in words, for its pill: "Pages is greater than 300", "Author contains “Le Guin”". */
export function filterText(filter: DatabaseFilter, property: DatabaseProperty): string {
	const { condition } = filter;
	const label = choiceOf(property.data_type, condition)?.label.toLowerCase() ?? condition.kind;
	if ('text' in condition) return `${property.name} ${label} “${condition.text}”`;
	if ('value' in condition && property.data_type !== 'checkbox') {
		const value =
			condition.value.type === 'text' ? `“${condition.value.value}”` : formatValue(condition.value);
		return `${property.name} ${label} ${value}`;
	}
	return `${property.name} ${label}`;
}

/** How a type orders, ascending then descending: "A to Z", "Z to A". */
export function sortDirections(type: DatabaseDataType): [string, string] {
	switch (type) {
		case 'text':
			return ['A to Z', 'Z to A'];
		case 'date':
			return ['Earliest first', 'Latest first'];
		case 'checkbox':
			return ['Unchecked first', 'Checked first'];
		default:
			return ['Lowest first', 'Highest first'];
	}
}
