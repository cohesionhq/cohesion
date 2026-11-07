import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createDeveloperStore, parseArgs, prettyJson } from './developer.svelte';

describe('developer store', () => {
	it('runs a script and pretty-prints its JSON result', async () => {
		const store = createDeveloperStore(createMock().send);
		store.script = '1 + 1';
		await store.run();
		expect(store.result).toBe('{\n  "ran": 5\n}');
		expect(store.runError).toBeNull();
	});

	it('keeps the console frame, and rejects args that are not JSON', async () => {
		const sent: unknown[] = [];
		const store = createDeveloperStore(createMock().send, async (request) => {
			sent.push(request);
			return { error: { code: 'badRequest', message: 'nope' } };
		});
		store.consoleType = 'about';
		store.consoleArgs = '{nope';
		await store.sendConsole();
		expect(store.consoleError).toMatch(/isn’t JSON/);
		expect(sent).toEqual([]);

		store.consoleArgs = '';
		await store.sendConsole();
		expect(sent).toEqual([{ type: 'about' }]);
		expect(JSON.parse(store.consoleFrame!)).toEqual({
			error: { code: 'badRequest', message: 'nope' }
		});
	});

	it('gets error replies from the mock as frames, not exceptions', async () => {
		const mock = createMock();
		const store = createDeveloperStore(mock.send, mock.sendRaw);
		store.consoleType = 'deleteClient';
		store.consoleArgs = '{ "id": 999 }';
		await store.sendConsole();
		expect(JSON.parse(store.consoleFrame!)).toMatchObject({ error: { code: 'notFound' } });

		store.consoleType = 'listWorkspaces';
		store.consoleArgs = '';
		await store.sendConsole();
		expect(JSON.parse(store.consoleFrame!).ok.data).toHaveLength(2);
	});

	it('pretty-prints JSON and passes other text through', () => {
		expect(prettyJson('[1]')).toBe('[\n  1\n]');
		expect(prettyJson('not json')).toBe('not json');
		expect(parseArgs('  ')).toEqual({ ok: true, value: undefined });
	});
});
