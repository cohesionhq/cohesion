import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { ProtocolError } from '$lib/protocol/errors';
import type { CatalogModel, Send } from '$lib/protocol/types';
import {
	cacheLifetime,
	createIntelligenceStore,
	emptyEndpointForm,
	endpointArgs,
	filterCatalog,
	formProvider,
	modelsToImport
} from './intelligence.svelte';

describe('cacheLifetime', () => {
	it('says whole hours as hours and the rest as minutes', () => {
		expect([5, 30, 60, 90, 720].map(cacheLifetime)).toEqual([
			'5 minutes',
			'30 minutes',
			'1 hour',
			'90 minutes',
			'12 hours'
		]);
	});
});

describe('endpoint form', () => {
	it('flattens a built-in preset and leaves out blanks', () => {
		const form = { ...emptyEndpointForm(), preset: 'openAi' as const, api_key: ' sk-1 ' };
		expect(endpointArgs(form)).toEqual({ preset: 'openAi', api_key: 'sk-1' });
	});

	it('sends a custom endpoint’s cache lifetime as a number, and never a preset’s', () => {
		const form = {
			...emptyEndpointForm(),
			preset: 'custom' as const,
			provider_identifier: 'proxy',
			base_url: 'http://localhost:8080/v1',
			cache_minutes: ' 60 '
		};
		expect(endpointArgs(form)).toMatchObject({ cache_minutes: 60 });
		expect(endpointArgs({ ...form, preset: 'openAi' })).toEqual({ preset: 'openAi' });
	});

	it('sends a long cache only with both its fields', () => {
		const form = {
			...emptyEndpointForm(),
			preset: 'custom' as const,
			provider_identifier: 'proxy',
			dialect: 'messages' as const,
			base_url: 'http://localhost:8080/v1',
			long_cache_minutes: '60'
		};
		expect(endpointArgs(form)).not.toHaveProperty('long_cache');
		expect(endpointArgs({ ...form, long_cache_write_multiple: ' 1.6 ' })).toMatchObject({
			long_cache: { minutes: 60, write_multiple: 1.6 }
		});
	});

	it('flattens a custom endpoint with its own fields', () => {
		const form = {
			...emptyEndpointForm(),
			preset: 'custom' as const,
			name: 'Local',
			provider_identifier: 'ollama',
			dialect: 'chatCompletions' as const,
			base_url: 'http://localhost:11434/v1 '
		};
		expect(endpointArgs(form)).toEqual({
			name: 'Local',
			preset: 'custom',
			provider_identifier: 'ollama',
			dialect: 'chatCompletions',
			base_url: 'http://localhost:11434/v1'
		});
	});

	it('reads models from the preset’s provider, or a custom endpoint’s own', () => {
		expect(formProvider({ ...emptyEndpointForm(), preset: 'fireworks' })).toBe('fireworks-ai');
		expect(
			formProvider({ ...emptyEndpointForm(), preset: 'custom', provider_identifier: ' ollama ' })
		).toBe('ollama');
	});

	it('creates through the store and lists the reply', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		const created = await store.createEndpoint({
			...emptyEndpointForm(),
			preset: 'custom',
			name: 'Local',
			provider_identifier: 'ollama',
			base_url: 'http://localhost:11434/v1'
		});
		expect(store.endpoints.value.at(-1)).toEqual(created);
		// A custom endpoint has no preset model to start with.
		expect(created.models).toEqual([]);
		await store.deleteEndpoint(created.id);
		expect(store.endpoints.value.some((e) => e.id === created.id)).toBe(false);
	});

	it('refuses a built-in preset without an API key', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		await expect(
			store.createEndpoint({ ...emptyEndpointForm(), preset: 'openAi' })
		).rejects.toMatchObject({
			code: 'validationFailed',
			message: 'This endpoint requires an API key.'
		});
		expect(store.endpoints.value).toHaveLength(3);
	});
});

describe('models to import', () => {
	it('adds the typed model and leaves out what the endpoint offers', () => {
		expect(modelsToImport(['a', 'b'], ' c ', ['b'])).toEqual(['a', 'c']);
		expect(modelsToImport(['a'], 'a', [])).toEqual(['a']);
		expect(modelsToImport([], '  ', [])).toEqual([]);
	});
});

describe('catalog filtering', () => {
	const row = (model_name: string, provider_name: string, input: number | null) =>
		({
			model_name,
			model_identifier: model_name.toLowerCase(),
			provider_name,
			provider_identifier: provider_name.toLowerCase(),
			context_size: null,
			input_token_cost: input,
			output_token_cost: null
		}) as CatalogModel;
	const models = [row('Beta', 'Zed', 2), row('Alpha', 'Zed', null), row('Gamma', 'Acme', 1)];

	it('matches name or provider and sorts by provider, then name', () => {
		expect(filterCatalog(models, '').map((m) => m.model_name)).toEqual(['Gamma', 'Alpha', 'Beta']);
		expect(filterCatalog(models, 'zed').map((m) => m.model_name)).toEqual(['Alpha', 'Beta']);
	});

	it('sorts by price with unknown prices last', () => {
		expect(filterCatalog(models, '', 'input').map((m) => m.model_name)).toEqual([
			'Gamma',
			'Beta',
			'Alpha'
		]);
		expect(filterCatalog(models, '', 'input', true).map((m) => m.model_name)).toEqual([
			'Beta',
			'Gamma',
			'Alpha'
		]);
	});
});

describe('intelligence store', () => {
	it('lists each endpoint with its models', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		expect(store.endpoints.value.map((e) => e.name)).toEqual(['Anthropic', 'OpenAI', 'XAI']);
		expect(store.endpoints.value[0].models.map((m) => m.model_identifier)).toEqual([
			'claude-haiku-5-5',
			'claude-sonnet-5-5'
		]);
	});

	it('starts a preset endpoint with its default model, and refuses a taken name', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		await expect(
			store.createEndpoint({ ...emptyEndpointForm(), preset: 'anthropic', api_key: 'k' })
		).rejects.toMatchObject({ code: 'validationFailed', field: 'name' });

		const work = await store.createEndpoint({
			...emptyEndpointForm(),
			preset: 'anthropic',
			name: 'Work',
			api_key: 'k'
		});
		expect(work.models.map((m) => m.model_identifier)).toEqual(['claude-sonnet-5-5']);
		// Each endpoint keeps its own list, though they share a provider.
		await store.importModels(work.id, ['claude-opus-5-5']);
		expect(store.endpoints.value.find((e) => e.id === work.id)?.models).toHaveLength(2);
		expect(store.endpoints.value[0].models).toHaveLength(2);
	});

	it('links a catalog model and stores one the catalog doesn’t list', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		await store.importModels(1, ['claude-opus-5-5', 'claude-next']);
		const models = store.endpoints.value[0].models;
		expect(models.map((m) => m.model_name)).toEqual([
			'Claude Haiku 5.5',
			'Claude Opus 5.5',
			'Claude Sonnet 5.5',
			'claude-next'
		]);
		expect(models.find((m) => m.model_identifier === 'claude-opus-5-5')).toMatchObject({
			input_token_cost: 4
		});
		expect(models.find((m) => m.model_identifier === 'claude-next')).toMatchObject({
			catalog_model_id: null,
			input_token_cost: null
		});
	});

	it('sets and clears tier defaults without refetching', async () => {
		const mock = createMock();
		const store = createIntelligenceStore(mock.send);
		await store.load();

		await store.setDefault('low', 2);
		expect(store.defaults.value.find((d) => d.tier === 'low')?.endpoint_model_id).toBe(2);
		await store.setDefault('agent', 2);
		expect(store.defaults.value.filter((d) => d.tier === 'agent')).toEqual([
			{ id: 1, tier: 'agent', endpoint_model_id: 2 }
		]);
		expect(mock.state.defaults.find((d) => d.tier === 'low')?.endpoint_model_id).toBe(2);

		await store.clearDefault('low');
		expect(store.defaults.value.map((d) => d.tier)).toEqual(['agent', 'medium', 'high']);
		expect(mock.state.defaults.map((d) => d.tier)).toEqual(['agent', 'medium', 'high']);
	});

	it('unsets the tiers of a removed model or a deleted endpoint', async () => {
		const mock = createMock();
		const store = createIntelligenceStore(mock.send);
		await store.load();
		await store.setDefault('low', 2);

		// Low and medium were on model 2.
		await store.removeModel(2);
		expect(store.endpoints.value[0].models.map((m) => m.id)).toEqual([1]);
		expect(store.defaults.value.map((d) => d.tier)).toEqual(['agent', 'high']);

		await store.deleteEndpoint(1);
		expect(store.endpoints.value.map((e) => e.name)).toEqual(['OpenAI', 'XAI']);
		expect(store.defaults.value).toEqual([]);
		expect(mock.state.defaults).toEqual([]);
	});

	it('lists a model the endpoint already offers once', async () => {
		const mock = createMock();
		const store = createIntelligenceStore(mock.send);
		await store.load();
		// Offered already, though this copy of the list doesn't know it.
		store.endpoints.update((list) => list.map((e) => ({ ...e, models: [] })));
		await store.importModels(1, ['claude-sonnet-5-5', 'claude-opus-5-5', 'claude-opus-5-5']);
		expect(store.endpoints.value[0].models.map((m) => m.model_identifier).sort()).toEqual([
			'claude-opus-5-5',
			'claude-sonnet-5-5'
		]);
		expect(mock.state.endpointModels.filter((m) => m.endpoint_id === 1)).toHaveLength(3);
	});

	it('lists a created endpoint even when its models can’t be read', async () => {
		const mock = createMock();
		let failing = false;
		const send = ((...args: Parameters<Send>) => {
			if (args[0] === 'listEndpointModels' && failing) {
				failing = false;
				return Promise.reject(new ProtocolError('disconnected', 'Unavailable'));
			}
			return mock.send(...args);
		}) as Send;
		const store = createIntelligenceStore(send);
		await store.load();
		failing = true;
		const work = await store.createEndpoint({
			...emptyEndpointForm(),
			preset: 'anthropic',
			name: 'Work',
			api_key: 'k'
		});
		expect(work.models).toEqual([]);
		expect(store.endpoints.value.some((e) => e.id === work.id)).toBe(true);
		// The list read again brings the preset's model.
		await store.endpoints.refetch();
		expect(store.endpoints.value.find((e) => e.id === work.id)?.models).toHaveLength(1);
	});

	it('refetches endpoints and defaults on an intelligence invalidation', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.load();
		for (const r of [store.endpoints, store.defaults]) {
			expect(r.touchedBy([{ kind: 'intelligence' }])).toBe(true);
			expect(r.touchedBy([{ kind: 'connectorAccounts' }])).toBe(false);
		}
	});

	it('narrows the catalog to one provider by key', async () => {
		const store = createIntelligenceStore(createMock().send);
		await store.catalog.load(null);
		// Only the providers of configured endpoints, until one is named.
		const providers = new Set(store.catalog.value.map((m) => m.provider_identifier));
		expect([...providers].sort()).toEqual(['anthropic', 'openai', 'xai']);
		await store.providerCatalog.load('openai');
		expect(store.providerCatalog.value.map((m) => m.model_identifier)).toEqual([
			'gpt-6-luna',
			'gpt-6.1-terra'
		]);
	});
});
