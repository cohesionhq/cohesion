// What the formula editor offers: a starting formula written with the
// database's own property names, and what a formula of each type returns.
// The daemon runs every formula and checks what it returns.

import type { DatabaseDataType, DatabaseProperty } from '$lib/protocol/types';

/** How a formula reads property `name` from its row: `row.Pages`, or `row["Unit price"]`. */
export function rowAccess(name: string): string {
	return /^[A-Za-z_$][\w$]*$/.test(name) ? `row.${name}` : `row[${JSON.stringify(name)}]`;
}

/** What a formula of `type` returns, as a sentence for its editor. */
export function returnHint(type: DatabaseDataType): string {
	switch (type) {
		case 'text':
			return 'Return text, or null to leave the cell empty.';
		case 'integer':
			return 'Return a whole number, or null to leave the cell empty.';
		case 'decimal':
			return 'Return a number, or null to leave the cell empty.';
		case 'currency':
			return 'Return an amount as a number, or null to leave the cell empty.';
		case 'checkbox':
			return 'Return true or false, or null to leave the cell empty.';
		case 'date':
			return 'Return a date written like 2026-11-10, or null to leave the cell empty.';
	}
}

/**
 * A formula of `type` to start from, reading the database's own properties
 * (those `properties` set by hand, but `except`, the one being made a formula).
 */
export function formulaStarter(
	type: DatabaseDataType,
	properties: DatabaseProperty[],
	except: number | null = null
): string {
	const inputs = properties.filter((p) => p.formula === null && p.id !== except);
	const of = (...types: DatabaseDataType[]) =>
		inputs.filter((p) => types.includes(p.data_type)).map((p) => rowAccess(p.name));
	const [number, other] = of('integer', 'decimal', 'currency');
	const [whole] = of('integer');
	const [text] = of('text');
	const [checkbox] = of('checkbox');
	const [date] = of('date');
	const body = (() => {
		switch (type) {
			case 'text':
				if (number) return `(${number} ?? 0) > 100 ? "High" : "Low"`;
				if (text) return `${text} ?? "None"`;
				return '"Hello"';
			case 'integer':
				if (whole) return `(${whole} ?? 0) * 2`;
				if (checkbox) return `${checkbox} ? 1 : 0`;
				return '1';
			case 'decimal':
			case 'currency':
				if (number && other) return `(${number} ?? 0) * (${other} ?? 0)`;
				if (number) return `(${number} ?? 0) * 1.2`;
				return '0';
			case 'checkbox':
				if (number) return `(${number} ?? 0) > 100`;
				if (text) return `${text} !== null`;
				return 'false';
			case 'date':
				return date ?? 'null';
		}
	})();
	return `(row) => ${body}`;
}
