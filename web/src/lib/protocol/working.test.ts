import { describe, expect, it } from 'vitest';
import type { Send } from './types';
import { working } from './working.svelte';

/** A `send` whose replies the test settles by hand. */
function pendingSend() {
	const settle: (() => void)[] = [];
	const send = (() =>
		new Promise<never>((resolve) => settle.push(() => resolve(undefined as never)))) as Send;
	return { send: working.track(send), settle };
}

describe('working', () => {
	it('is acting while a request is in flight', async () => {
		const { send, settle } = pendingSend();
		const reply = send('listWorkspaces');
		expect(working.acting).toBe(true);
		expect(working.loading).toBe(false);
		settle[0]();
		await reply;
		expect(working.acting).toBe(false);
	});

	it('counts what a load sends before its first await as loading', async () => {
		const { send, settle } = pendingSend();
		const load = working.load(() => send('listWorkspaces'));
		expect(working.loading).toBe(true);
		expect(working.acting).toBe(false);
		const action = send('listWorkspaces');
		expect(working.acting).toBe(true);
		settle.forEach((s) => s());
		await Promise.all([load, action]);
		expect(working.loading).toBe(false);
		expect(working.acting).toBe(false);
	});
});
