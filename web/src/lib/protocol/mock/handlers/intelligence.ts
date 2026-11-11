// Model endpoints, the catalog, defaults, spend and threads.

import { ProtocolError } from '../../errors';
import type { RedactedIntelligenceEndpoint } from '../../types';
import { PRESET_DEFAULTS, spend } from '../seed';
import type { Handlers, MockCore } from '../core';

export function intelligenceHandlers(core: MockCore) {
	const { state, nextId, notFound, modelsOf, dropModels, addEndpointModel } = core;
	return {
		updateModelCatalog: () => null,
		listIntelligenceEndpoints: () => state.endpoints.map((e) => structuredClone(e)),
		createIntelligenceEndpoint: (args) => {
			if (args.preset !== 'custom' && !args.api_key)
				throw new ProtocolError('validationFailed', 'This endpoint requires an API key.');
			const defaults =
				args.preset === 'custom'
					? {
							name: args.provider_identifier,
							provider_identifier: args.provider_identifier,
							dialect: args.dialect,
							default_model: null,
							base_url: args.base_url,
							usage_url: null,
							cache_minutes: 5,
							long_cache: null
						}
					: PRESET_DEFAULTS[args.preset];
			const name = args.name ?? defaults.name;
			if (state.endpoints.some((e) => e.name.toLowerCase() === name.toLowerCase()))
				throw new ProtocolError(
					'validationFailed',
					`An endpoint named ${name} already exists; give this one another name.`,
					'name'
				);
			const created: RedactedIntelligenceEndpoint = {
				id: nextId(state.endpoints),
				name,
				backend: {
					kind: 'integrated',
					provider_identifier: defaults.provider_identifier,
					dialect: defaults.dialect,
					base_url: defaults.base_url,
					usage_url: args.usage_url ?? defaults.usage_url,
					cache_minutes: args.cache_minutes ?? defaults.cache_minutes,
					long_cache: args.long_cache ?? defaults.long_cache
				}
			};
			state.endpoints.push(created);
			if (defaults.default_model !== null)
				addEndpointModel({ endpoint_id: created.id, model_identifier: defaults.default_model });
			return structuredClone(created);
		},
		deleteIntelligenceEndpoint: ({ id }) => {
			const index = state.endpoints.findIndex((e) => e.id === id);
			if (index < 0) notFound('intelligence endpoint');
			const endpoint = structuredClone(state.endpoints[index]);
			dropModels((m) => m.endpoint_id === id);
			state.endpoints.splice(index, 1);
			return endpoint;
		},
		setIntelligenceEndpointUsageUrl: ({ id, usage_url }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, usage_url };
			return structuredClone(endpoint);
		},
		setIntelligenceEndpointCacheMinutes: ({ id, cache_minutes }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, cache_minutes };
			return structuredClone(endpoint);
		},
		setIntelligenceEndpointLongCache: ({ id, long_cache }) => {
			const endpoint =
				state.endpoints.find((e) => e.id === id) ?? notFound('intelligence endpoint');
			endpoint.backend = { ...endpoint.backend, long_cache: long_cache ?? null };
			return structuredClone(endpoint);
		},
		addEndpointModel,
		removeEndpointModel: ({ id }) => {
			const model = state.endpointModels.find((m) => m.id === id) ?? notFound('endpoint model');
			dropModels((m) => m.id === id);
			return { ...model };
		},
		listEndpointModels: ({ endpoint_id }) => modelsOf(endpoint_id).map((m) => ({ ...m })),
		setIntelligenceDefault: ({ tier, endpoint_model_id }) => {
			if (!state.endpointModels.some((m) => m.id === endpoint_model_id))
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			const existing = state.defaults.find((d) => d.tier === tier);
			if (existing) existing.endpoint_model_id = endpoint_model_id;
			else state.defaults.push({ id: nextId(state.defaults), tier, endpoint_model_id });
			return null;
		},
		clearIntelligenceDefault: ({ tier }) => {
			state.defaults = state.defaults.filter((d) => d.tier !== tier);
			return null;
		},
		listCatalogModels: ({ provider_identifier, query }) => {
			const providers =
				provider_identifier == null
					? new Set(
							state.endpoints.flatMap((e) =>
								e.backend.kind === 'integrated' ? [e.backend.provider_identifier] : []
							)
						)
					: new Set([provider_identifier]);
			const q = query?.toLowerCase() ?? '';
			return state.catalog
				.filter(
					(m) =>
						providers.has(m.provider_identifier) &&
						(m.model_identifier.toLowerCase().includes(q) || m.model_name.toLowerCase().includes(q))
				)
				.sort(
					(a, b) =>
						a.provider_identifier.localeCompare(b.provider_identifier) ||
						a.model_identifier.localeCompare(b.model_identifier)
				)
				.map((m) => ({ ...m }));
		},
		listIntelligenceSpend: () => spend(state),
		setIntelligenceSpendLimit: ({ period, amount }) => {
			if (!(amount > 0)) throw new ProtocolError('validationFailed', 'Validation failed.');
			const existing = state.spendLimits.find((l) => l.period === period);
			if (existing) existing.amount = amount;
			else state.spendLimits.push({ period, amount });
			return spend(state);
		},
		clearIntelligenceSpendLimit: ({ period }) => {
			state.spendLimits = state.spendLimits.filter((l) => l.period !== period);
			return spend(state);
		},
		listIntelligenceCostEvents: ({ limit }) =>
			[...state.costEvents]
				.reverse()
				.slice(0, limit)
				.map((e) => ({ ...e })),
		getIntelligenceThread: ({ thread_id }) => {
			const thread =
				state.threads.find((t) => t.id === thread_id) ?? notFound('intelligence thread');
			return {
				thread: { ...thread },
				messages: state.messages.filter((m) => m.thread_id === thread_id).map((m) => ({ ...m })),
				cost_events: state.costEvents
					.filter((e) => e.thread_id === thread_id)
					.map((e) => ({ ...e }))
			};
		}
	} satisfies Partial<Handlers>;
}
