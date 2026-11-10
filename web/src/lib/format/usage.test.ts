import { describe, expect, it } from 'vitest';
import type { IntelligenceCostEvent } from '$lib/protocol/types';
import { costParts, tokenCount, usageCsv, usageTotals } from './usage';

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

describe('usageCsv', () => {
	const header =
		'time,provider,model,thread_id,input_tokens,cache_write_tokens,cache_read_tokens,output_tokens,' +
		'input_cost_usd,cache_write_cost_usd,cache_read_cost_usd,output_cost_usd,total_cost_usd';

	it('writes a header and one row per call, oldest first, with raw counts and unrounded costs', () => {
		const csv = usageCsv([
			event({
				id: 2,
				created_at: 1_794_000_100,
				thread_id: 7,
				input_tokens: 120,
				cache_write_tokens: 30,
				cache_read_tokens: 900,
				output_tokens: 45,
				input_cost: 0.00036,
				cache_write_cost: 0.0001125,
				cache_read_cost: 0.00027,
				output_cost: 0.000675,
				total_cost: 0.0014175
			}),
			event({ id: 1, created_at: 1_794_000_000, total_cost: 0.5 })
		]);
		expect(csv.split('\r\n')).toEqual([
			header,
			'2026-11-06T21:20:00.000Z,anthropic,claude-sonnet-4-5,,0,0,0,0,,,,,0.5',
			'2026-11-06T21:21:40.000Z,anthropic,claude-sonnet-4-5,7,120,30,900,45,0.00036,0.0001125,0.00027,0.000675,0.0014175',
			''
		]);
	});

	it('quotes a field holding a comma or a quote', () => {
		const csv = usageCsv([event({ model_identifier: 'a,"b"' })]);
		expect(csv.split('\r\n')[1]).toContain(',"a,""b""",');
	});

	it('is only the header with no calls', () => {
		expect(usageCsv([])).toBe(`${header}\r\n`);
	});
});
