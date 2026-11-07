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
