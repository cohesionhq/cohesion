// Model usage, read from intelligence cost events.

import type { IntelligenceCostEvent } from '$lib/protocol/types';

/** "850", "12K", "1.2M". */
export function tokenCount(count: number): string {
	if (count >= 1_000_000) return `${Number((count / 1_000_000).toFixed(1))}M`;
	if (count >= 1_000) return `${Math.round(count / 1_000)}K`;
	return String(count);
}

/** False for calls recorded before token counts were kept; they show only a cost. */
export function hasTokenCounts(event: IntelligenceCostEvent): boolean {
	return inputTokens(event) + event.cache_read_tokens + event.output_tokens > 0;
}

/** Prompt tokens a call or run paid full price or more for: everything not read from the cache. */
export function inputTokens(
	usage: Pick<IntelligenceCostEvent, 'input_tokens' | 'cache_write_tokens'>
): number {
	return usage.input_tokens + usage.cache_write_tokens;
}

export interface CostPart {
	label: string;
	cost: number;
}

/** What each kind of token cost in a call: none when its provider reported only the total. */
export function costParts(event: IntelligenceCostEvent): CostPart[] {
	const parts: [string, number | null][] = [
		['Input', event.input_cost],
		['Output', event.output_cost],
		['Cache reads', event.cache_read_cost],
		['Cache writes', event.cache_write_cost]
	];
	return parts.flatMap(([label, cost]) => (cost === null ? [] : [{ label, cost }]));
}

export interface UsageTotals {
	calls: number;
	input_tokens: number;
	cached_tokens: number;
	output_tokens: number;
	cost: number;
}

export function usageTotals(events: IntelligenceCostEvent[]): UsageTotals {
	return events.reduce<UsageTotals>(
		(totals, event) => ({
			calls: totals.calls + 1,
			input_tokens: totals.input_tokens + inputTokens(event),
			cached_tokens: totals.cached_tokens + event.cache_read_tokens,
			output_tokens: totals.output_tokens + event.output_tokens,
			cost: totals.cost + event.total_cost
		}),
		{ calls: 0, input_tokens: 0, cached_tokens: 0, output_tokens: 0, cost: 0 }
	);
}

/** The columns of a usage export, each with how to read it from a cost event. */
const CSV_COLUMNS: [string, (event: IntelligenceCostEvent) => string | number | null][] = [
	['time', (e) => new Date(e.created_at * 1000).toISOString()],
	['provider', (e) => e.provider_identifier],
	['model', (e) => e.model_identifier],
	['thread_id', (e) => e.thread_id],
	['input_tokens', (e) => e.input_tokens],
	['cache_write_tokens', (e) => e.cache_write_tokens],
	['cache_read_tokens', (e) => e.cache_read_tokens],
	['output_tokens', (e) => e.output_tokens],
	['input_cost_usd', (e) => e.input_cost],
	['cache_write_cost_usd', (e) => e.cache_write_cost],
	['cache_read_cost_usd', (e) => e.cache_read_cost],
	['output_cost_usd', (e) => e.output_cost],
	['total_cost_usd', (e) => e.total_cost]
];

/** One CSV field: quoted when it holds a comma, quote or line break; empty for null. */
function csvField(value: string | number | null): string {
	if (value === null) return '';
	const text = String(value);
	return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

/**
 * Every call as a CSV row (RFC 4180), oldest first, for analysis elsewhere. Times are UTC in
 * ISO 8601; token counts are as the provider reported them, so `input_tokens` leaves out cache
 * writes; a cost a provider didn't break down is empty, and costs are in US dollars, unrounded.
 */
export function usageCsv(events: IntelligenceCostEvent[]): string {
	const rows = [...events]
		.sort((a, b) => a.created_at - b.created_at || a.id - b.id)
		.map((event) => CSV_COLUMNS.map(([, read]) => csvField(read(event))).join(','));
	return [CSV_COLUMNS.map(([name]) => name).join(','), ...rows].map((row) => `${row}\r\n`).join('');
}
