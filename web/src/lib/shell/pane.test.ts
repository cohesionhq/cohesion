import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SplitPane } from './pane.svelte';

vi.mock('$app/navigation', () => ({ beforeNavigate: vi.fn(), goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: {} }));
vi.mock('./splitRoutes', () => ({
	resolveRoute: async (href: string) => ({
		url: new URL(href, 'http://localhost'),
		params: {},
		route: { id: null }
	})
}));

describe('SplitPane history', () => {
	let split: SplitPane;

	beforeEach(() => {
		vi.stubGlobal('location', { origin: 'http://localhost' });
		vi.stubGlobal('sessionStorage', { setItem: vi.fn(), removeItem: vi.fn() });
		split = new SplitPane();
	});

	it('has no way back from the first page it opens', async () => {
		await split.goto('/actors/1');
		expect(split.state.back).toBeUndefined();
	});

	it('names the page it came from, and goes back to it', async () => {
		await split.goto('/agents/1');
		split.setTitle('Scout · Agents');
		await split.goto('/actors/1');
		expect(split.state.back).toEqual({ href: '/agents/1', label: 'Scout' });

		split.step(-1);
		expect(split.href).toBe('/agents/1');
		expect(split.state.back).toBeUndefined();
	});

	it('keeps a replaced entry’s way back, which is the one history still has', async () => {
		await split.goto('/agents/1');
		split.setTitle('Scout · Agents');
		await split.goto('/open/actor-session/1');
		split.setTitle('Cohesion');
		await split.goto('/actors/1', { replaceState: true });
		expect(split.state.back).toEqual({ href: '/agents/1', label: 'Scout' });

		split.step(-1);
		expect(split.href).toBe('/agents/1');
	});

	it('leads nowhere from a replaced first page, whose entry is the only one', async () => {
		await split.goto('/open/actor-session/1');
		split.setTitle('Cohesion');
		await split.goto('/actors/1', { replaceState: true });
		expect(split.state.back).toBeUndefined();

		split.step(-1);
		expect(split.href).toBe('/actors/1');
	});

	it('treats a query change as the same page', async () => {
		await split.goto('/w/1/tasks');
		await split.goto('/w/1/tasks?view=all');
		expect(split.state.back).toBeUndefined();
	});
});
