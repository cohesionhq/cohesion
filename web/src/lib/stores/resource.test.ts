import { describe, expect, it } from 'vitest';
import { Resource, refreshAll, refreshInvalidated } from './resource.svelte';

/** A resource over a counter, so a test can see each fetch. */
function counted() {
	let fetches = 0;
	const resource = new Resource<number, number>({
		fetch: async () => ++fetches,
		initial: 0,
		matches: (key, id) => key.kind === 'workspace' && key.id === id
	});
	return { resource, fetches: () => fetches };
}

describe('Resource', () => {
	it('a new key empties the value unless keepsValue lets it show until the new one lands', async () => {
		const resource = new Resource<string, { list: number; filter: string }>({
			fetch: async ({ list, filter }) => `${list}:${filter}`,
			initial: '',
			keepsValue: (from, to) => from.list === to.list
		});
		await resource.load({ list: 1, filter: 'a' });

		const narrowing = resource.load({ list: 1, filter: 'b' });
		expect(resource.value).toBe('1:a');
		expect(resource.loaded).toBe(true);
		await narrowing;
		expect(resource.value).toBe('1:b');

		const switching = resource.load({ list: 2, filter: 'b' });
		expect(resource.value).toBe('');
		expect(resource.loaded).toBe(false);
		await switching;
		expect(resource.value).toBe('2:b');
	});

	it('loads a key once and refetches it in the background while active', async () => {
		const { resource, fetches } = counted();
		await resource.load(1);
		await resource.load(1);
		expect(fetches()).toBe(1);

		await refreshAll();
		expect(fetches()).toBe(2);
		await refreshInvalidated([{ kind: 'workspace', id: 2 }]);
		expect(fetches()).toBe(2);
		await refreshInvalidated([{ kind: 'workspace', id: 1 }]);
		expect(fetches()).toBe(3);
		expect(resource.value).toBe(3);
	});

	it('a released resource keeps its value and rejoins background refetch on the next load', async () => {
		const { resource, fetches } = counted();
		await resource.load(1);
		resource.release();
		await refreshInvalidated([{ kind: 'workspace', id: 2 }]);
		expect(fetches()).toBe(1);

		// Nothing touched it while released, so this fetches nothing but makes it active again.
		await resource.load(1);
		expect(fetches()).toBe(1);
		await refreshAll();
		expect(fetches()).toBe(2);
		await refreshInvalidated([{ kind: 'workspace', id: 1 }]);
		expect(fetches()).toBe(3);
	});

	it('a released resource refetches on the next load after missing an invalidation', async () => {
		const { resource, fetches } = counted();
		await resource.load(1);
		resource.release();
		await refreshInvalidated([{ kind: 'workspace', id: 1 }]);
		expect(fetches()).toBe(1);
		expect(resource.value).toBe(1);

		await resource.load(1);
		expect(fetches()).toBe(2);
		await resource.load(1);
		expect(fetches()).toBe(2);
	});

	it('a released resource refetches on the next load after missing a refresh', async () => {
		const { resource, fetches } = counted();
		await resource.load(1);
		resource.release();
		await refreshAll();
		expect(fetches()).toBe(1);

		await resource.load(1);
		expect(fetches()).toBe(2);
	});
});
