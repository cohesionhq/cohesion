import { describe, expect, it } from 'vitest';
import { batched, groupBy } from './batch';

describe('batched', () => {
	it('asks for every key wanted in the same task in one call, each once', async () => {
		const calls: number[][] = [];
		const fetchOne = batched(
			async (keys: number[]) => {
				calls.push(keys);
				return new Map(keys.filter((k) => k !== 3).map((k) => [k, k * 10]));
			},
			() => 0
		);
		const values = await Promise.all([fetchOne(1), fetchOne(2), fetchOne(1), fetchOne(3)]);
		expect(values).toEqual([10, 20, 10, 0]);
		expect(calls).toEqual([[1, 2, 3]]);

		expect(await fetchOne(4)).toBe(40);
		expect(calls).toEqual([[1, 2, 3], [4]]);
	});

	it('fails every caller of a failed call', async () => {
		const fetchOne = batched<number, number>(
			() => Promise.reject(new Error('down')),
			() => 0
		);
		const results = await Promise.allSettled([fetchOne(1), fetchOne(2)]);
		expect(results.map((r) => r.status)).toEqual(['rejected', 'rejected']);
	});
});

describe('groupBy', () => {
	it('groups rows by their key, in order', () => {
		const rows = [
			{ id: 1, of: 'a' },
			{ id: 2, of: 'b' },
			{ id: 3, of: 'a' }
		];
		expect(groupBy(rows, (r) => r.of)).toEqual(
			new Map([
				['a', [rows[0], rows[2]]],
				['b', [rows[1]]]
			])
		);
	});
});
