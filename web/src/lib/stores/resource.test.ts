import { describe, expect, it } from 'vitest';
import { Resource, firstFetchesSettled, refreshAll, refreshInvalidated } from './resource.svelte';

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

	it('a write applied while a fetch waits wins over that fetch, which may have read before it', async () => {
		const answers: ((value: string) => void)[] = [];
		const resource = new Resource<string, number>({
			fetch: () => new Promise((resolve) => answers.push(resolve)),
			initial: ''
		});
		const loading = resource.load(1);
		answers[0]('old');
		await loading;

		const before = resource.refetch();
		resource.set('new');
		expect(answers).toHaveLength(3);
		answers[1]('old');
		await before;
		expect(resource.value).toBe('new');

		// The fetch the write started read after it, so its value goes on.
		answers[2]('newer');
		await new Promise((resolve) => setTimeout(resolve));
		expect(resource.value).toBe('newer');
	});

	it('a write with no fetch waiting fetches nothing', async () => {
		const { resource, fetches } = counted();
		await resource.load(1);
		resource.set(10);
		resource.update((n) => n + 1);
		expect(fetches()).toBe(1);
		expect(resource.value).toBe(11);
	});

	it('a first fetch that outlasts a page fade’s wait goes on through its reveal', async () => {
		let answer: (value: string) => void = () => undefined;
		const late = new Resource<string, number>({
			fetch: () => new Promise((resolve) => (answer = resolve)),
			initial: ''
		});
		const quick = new Resource<string, number>({ fetch: async () => 'quick', initial: '' });
		const shows: (() => void)[] = [];
		const reveal = (show: () => void) => {
			shows.push(show);
			return Promise.resolve();
		};

		const loading = late.load(1);
		await quick.load(1);
		await firstFetchesSettled(5, reveal);
		expect(quick.value).toBe('quick');

		answer('late');
		await loading;
		expect(shows).toHaveLength(1);
		expect(late.loaded).toBe(false);
		shows[0]();
		expect(late.value).toBe('late');
		expect(late.loaded).toBe(true);
	});

	it('a first fetch that lands within the wait goes on at once', async () => {
		let answer: (value: string) => void = () => undefined;
		const resource = new Resource<string, number>({
			fetch: () => new Promise((resolve) => (answer = resolve)),
			initial: ''
		});
		let revealed = false;
		const loading = resource.load(1);
		const waiting = firstFetchesSettled(1000, async (show) => {
			revealed = true;
			show();
		});
		answer('soon');
		await waiting;
		await loading;
		expect(resource.value).toBe('soon');
		expect(revealed).toBe(false);
	});
});
