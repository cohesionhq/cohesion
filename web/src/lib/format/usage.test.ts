import { describe, expect, it } from 'vitest';
import type { IntelligenceCostEvent } from '$lib/protocol/types';
import { costParts, tokenCount, usageTotals } from './usage';

function event(fields: Partial<IntelligenceCostEvent>): IntelligenceCostEvent {
	return {
		id: 1,
		created_at: 0,
		thread_id: null,
		provider_identifier: 'anthropic',
		model_identifier: 'claude-sonnet-4-5',
		input_tokens: 0,
		output_tokens: 0,
		cache_read_tokens: 0,
		cache_write_tokens: 0,
		total_cost: 0,
		input_cost: null,
		output_cost: null,
		cache_read_cost: null,
		cache_write_cost: null,
		...fields
	};
}

describe('usageTotals', () => {
	it('keeps cached prompt tokens apart from the rest', () => {
		const totals = usageTotals([
			event({ input_tokens: 100, cache_read_tokens: 900, output_tokens: 50, total_cost: 0.25 }),
			event({ cache_write_tokens: 10, total_cost: 0.75 })
		]);
		expect(totals).toEqual({
			calls: 2,
			input_tokens: 110,
			cached_tokens: 900,
			output_tokens: 50,
			cost: 1
		});
	});
});

describe('costParts', () => {
	it('lists the parts a call reported and skips the rest', () => {
		expect(costParts(event({ total_cost: 1 }))).toEqual([]);
		expect(costParts(event({ input_cost: 0.25, output_cost: 0.5, cache_read_cost: 0 }))).toEqual([
			{ label: 'Input', cost: 0.25 },
			{ label: 'Output', cost: 0.5 },
			{ label: 'Cache reads', cost: 0 }
		]);
	});
});

describe('tokenCount', () => {
	it('abbreviates thousands and millions', () => {
		expect(tokenCount(850)).toBe('850');
		expect(tokenCount(12_400)).toBe('12K');
		expect(tokenCount(1_250_000)).toBe('1.3M');
	});
});
