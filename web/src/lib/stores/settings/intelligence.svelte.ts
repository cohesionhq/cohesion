// Settings → Agents: endpoints and the models each offers, tier
// defaults, which point at one of those models, and the model catalog the
// models are imported from. An endpoint an extension set up with one of its
// accounts, such as a ChatGPT sign-in, goes when the account is removed.

import type {
	CatalogModel,
	CreateIntelligenceEndpointRequest,
	EndpointModel,
	IntelligenceDefault,
	IntelligenceEndpointPreset,
	IntelligenceTier,
	Invalidation,
	RedactedEndpointBackend,
	RedactedIntelligenceEndpoint,
	Send
} from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import { Resource } from '../resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export type PresetName = IntelligenceEndpointPreset['preset'];
export type Dialect = Extract<IntelligenceEndpointPreset, { preset: 'custom' }>['dialect'];

/**
 * Presets in menu order, with the name a person knows them by and, for a
 * built-in one, the catalog provider it reads models from.
 */
export function presets(): { value: PresetName; label: string; provider: string | null }[] {
	return [
		...vocabulary.value.providers.map((p) => ({
			value: p.kind,
			label: p.label,
			provider: p.provider_identifier
		})),
		{ value: 'custom', label: 'Custom', provider: null }
	];
}

/** What to call an endpoint's provider: the preset's name, or a custom endpoint's identifier. */
export function providerLabel(provider_identifier: string): string {
	return vocabulary.provider(provider_identifier) ?? provider_identifier;
}

export interface EndpointForm {
	preset: PresetName;
	name: string;
	api_key: string;
	provider_identifier: string;
	dialect: Dialect;
	base_url: string;
	usage_url: string;
	/** A custom endpoint's cache: whole minutes, or blank for 5. A preset's comes with its provider. */
	cache_minutes: string;
	/** A custom endpoint's long cache in whole minutes, given with `long_cache_write_multiple`, or blank for none. */
	long_cache_minutes: string;
	/** A multiple of the cache write price, given with `long_cache_minutes`. */
	long_cache_write_multiple: string;
}

export function emptyEndpointForm(): EndpointForm {
	return {
		preset: 'anthropic',
		name: '',
		api_key: '',
		provider_identifier: '',
		dialect: 'chatCompletions',
		base_url: '',
		usage_url: '',
		cache_minutes: '',
		long_cache_minutes: '',
		long_cache_write_multiple: ''
	};
}

/** The catalog provider a form's models come from: the preset's, or a custom endpoint's own. */
export function formProvider(form: EndpointForm): string {
	return (
		presets().find((p) => p.value === form.preset)?.provider ?? form.provider_identifier.trim()
	);
}

/** Built-in presets need a key; a custom endpoint may run without one. */
export function requiresApiKey(preset: PresetName): boolean {
	return preset !== 'custom';
}

/** The `createIntelligenceEndpoint` args for a form: the preset flattened in, blanks left out. */
export function endpointArgs(form: EndpointForm): CreateIntelligenceEndpointRequest {
	const name = form.name.trim();
	const api_key = form.api_key.trim();
	const usage_url = form.usage_url.trim();
	const cache_minutes = form.cache_minutes.trim();
	const long_minutes = form.long_cache_minutes.trim();
	const long_multiple = form.long_cache_write_multiple.trim();
	const common = {
		...(name === '' ? {} : { name }),
		...(api_key === '' ? {} : { api_key }),
		...(usage_url === '' ? {} : { usage_url })
	};
	if (form.preset === 'custom')
		return {
			...common,
			// A preset's provider comes with its cache; a custom endpoint describes its own.
			...(cache_minutes === '' ? {} : { cache_minutes: Number(cache_minutes) }),
			...(long_minutes === '' || long_multiple === ''
				? {}
				: {
						long_cache: { minutes: Number(long_minutes), write_multiple: Number(long_multiple) }
					}),
			preset: 'custom',
			provider_identifier: form.provider_identifier.trim(),
			dialect: form.dialect,
			base_url: form.base_url.trim()
		};
	return { ...common, preset: form.preset };
}

/** A cache lifetime in words: whole hours as hours ("1 hour", "12 hours"), anything else as minutes. */
export function cacheLifetime(minutes: number): string {
	const [count, unit] = minutes % 60 === 0 ? [minutes / 60, 'hour'] : [minutes, 'minute'];
	return `${count} ${unit}${count === 1 ? '' : 's'}`;
}

/** An endpoint with the models it offers, by name: the unit the endpoint list and the tier pickers show. */
export interface EndpointWithModels extends RedactedIntelligenceEndpoint {
	models: EndpointModel[];
}

/** An endpoint that signs in with an extension's account, and goes with it. */
export type AccountEndpoint = EndpointWithModels & {
	backend: Extract<RedactedEndpointBackend, { kind: 'account' }>;
};

/** Whether `endpoint` signs in with an account rather than an API key. */
export function signsInWithAccount(endpoint: EndpointWithModels): endpoint is AccountEndpoint {
	return endpoint.backend.kind === 'account';
}

/** Endpoint `endpoint_id` with `change` applied to its models, kept in the daemon's order. */
function withModels(
	endpoints: EndpointWithModels[],
	endpoint_id: number,
	change: (models: EndpointModel[]) => EndpointModel[]
): EndpointWithModels[] {
	return endpoints.map((e) =>
		e.id === endpoint_id ? { ...e, models: change(e.models).sort(byModelName) } : e
	);
}

/** By name, then identifier, in code-unit order, as the daemon's `ORDER BY` sorts them. */
function byModelName(a: EndpointModel, b: EndpointModel): number {
	const key = (m: EndpointModel) => [m.model_name, m.model_identifier];
	const [x, y] = [key(a), key(b)];
	for (let i = 0; i < x.length; i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1;
	return 0;
}

/**
 * The model identifiers to import: the checked catalog models, then the one
 * typed by hand, if any, leaving out what the endpoint already offers.
 */
export function modelsToImport(checked: string[], typed: string, imported: string[]): string[] {
	const wanted = [...checked, typed.trim()].filter((m) => m !== '');
	return wanted.filter((m, i) => wanted.indexOf(m) === i && !imported.includes(m));
}

export type CatalogSort = 'name' | 'provider' | 'context' | 'input' | 'output';

/** Catalog rows matching `query` (name, identifier or provider), sorted by `sort`. */
export function filterCatalog(
	models: CatalogModel[],
	query: string,
	sort: CatalogSort = 'provider',
	descending = false
): CatalogModel[] {
	const q = query.trim().toLowerCase();
	const matching =
		q === ''
			? [...models]
			: models.filter((m) =>
					[m.model_name, m.model_identifier, m.provider_name, m.provider_identifier].some((s) =>
						s.toLowerCase().includes(q)
					)
				);
	const byName = (a: CatalogModel, b: CatalogModel) => a.model_name.localeCompare(b.model_name);
	const number =
		(key: 'context_size' | 'input_token_cost' | 'output_token_cost') =>
		(a: CatalogModel, b: CatalogModel) => {
			const x = a[key];
			const y = b[key];
			// Unknown values sort last in either direction.
			if (x === null || y === null) return x === y ? 0 : x === null ? 1 : -1;
			return (descending ? y - x : x - y) || byName(a, b);
		};
	const compare = {
		name: (a: CatalogModel, b: CatalogModel) => (descending ? -1 : 1) * byName(a, b),
		provider: (a: CatalogModel, b: CatalogModel) =>
			(descending ? -1 : 1) * a.provider_name.localeCompare(b.provider_name) || byName(a, b),
		context: number('context_size'),
		input: number('input_token_cost'),
		output: number('output_token_cost')
	}[sort];
	return matching.sort(compare);
}

export class IntelligenceStore {
	readonly endpoints: Resource<EndpointWithModels[]>;
	readonly defaults: Resource<IntelligenceDefault[]>;
	/** Keyed by the provider it is narrowed to, or null for every endpoint's. */
	readonly catalog: Resource<CatalogModel[], string | null>;
	/** One provider's catalog models, for the import checklists (the table may be narrowed otherwise). */
	readonly providerCatalog: Resource<CatalogModel[], string>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		const matches = (key: Invalidation) => key.kind === 'intelligence';
		this.endpoints = new Resource({
			fetch: async () => {
				const endpoints = await send('listIntelligenceEndpoints');
				const models = await Promise.all(
					endpoints.map((e) => send('listEndpointModels', { endpoint_id: e.id }))
				);
				return endpoints.map((e, i) => ({ ...e, models: models[i] }));
			},
			initial: [],
			matches
		});
		this.defaults = new Resource({
			fetch: () => send('listIntelligenceDefaults'),
			initial: [],
			matches
		});
		this.catalog = new Resource({
			fetch: (provider_identifier) =>
				send('listCatalogModels', provider_identifier === null ? {} : { provider_identifier }),
			initial: []
		});
		this.providerCatalog = new Resource({
			fetch: (provider_identifier) => send('listCatalogModels', { provider_identifier }),
			initial: []
		});
	}

	load(): Promise<void> {
		return Promise.all([this.endpoints.load(), this.defaults.load()]).then(() => undefined);
	}

	/** Stops background refetches of everything `load` fetched and the catalogs. */
	release(): void {
		for (const r of [this.endpoints, this.defaults, this.catalog, this.providerCatalog])
			r.release();
	}

	/**
	 * Creates an endpoint. The daemon imports the preset's default model with
	 * it, which the reply doesn't carry, so its models are listed after; if
	 * that fails, the endpoint is listed without them and read again.
	 */
	async createEndpoint(form: EndpointForm): Promise<EndpointWithModels> {
		const created = await this.#send('createIntelligenceEndpoint', endpointArgs(form));
		let endpoint: EndpointWithModels;
		try {
			const models = await this.#send('listEndpointModels', { endpoint_id: created.id });
			endpoint = { ...created, models };
		} catch {
			endpoint = { ...created, models: [] };
			void this.endpoints.refetch();
		}
		this.endpoints.update((list) => [...list, endpoint]);
		return endpoint;
	}

	/**
	 * Deletes an endpoint; its models go with it, and the daemon unsets the
	 * tiers that used them, so the defaults are read again.
	 */
	async deleteEndpoint(id: number): Promise<RedactedIntelligenceEndpoint> {
		const deleted = await this.#send('deleteIntelligenceEndpoint', { id });
		this.endpoints.update((list) => list.filter((e) => e.id !== deleted.id));
		await this.defaults.refetch();
		return deleted;
	}

	/**
	 * Imports each of `model_identifiers` into endpoint `endpoint_id`, in
	 * order. One the endpoint already offers comes back as it is.
	 */
	async importModels(endpoint_id: number, model_identifiers: string[]): Promise<void> {
		for (const model_identifier of model_identifiers) {
			const added = await this.#send('addEndpointModel', { endpoint_id, model_identifier });
			this.endpoints.update((list) =>
				withModels(list, endpoint_id, (m) => [...m.filter((x) => x.id !== added.id), added])
			);
		}
	}

	/**
	 * Removes an endpoint's model; the daemon unsets the tiers that used it,
	 * so the defaults are read again.
	 */
	async removeModel(id: number): Promise<EndpointModel> {
		const removed = await this.#send('removeEndpointModel', { id });
		this.endpoints.update((list) =>
			withModels(list, removed.endpoint_id, (m) => m.filter((x) => x.id !== removed.id))
		);
		await this.defaults.refetch();
		return removed;
	}

	/** Sets `tier`'s default. The reply carries nothing, so the row is built from the request. */
	async setDefault(tier: IntelligenceTier, endpoint_model_id: number): Promise<void> {
		await this.#send('setIntelligenceDefault', { tier, endpoint_model_id });
		this.defaults.update((list) => {
			const existing = list.find((d) => d.tier === tier);
			const row: IntelligenceDefault = { id: existing?.id ?? 0, tier, endpoint_model_id };
			return existing ? list.map((d) => (d.tier === tier ? row : d)) : [...list, row];
		});
	}

	async clearDefault(tier: IntelligenceTier): Promise<void> {
		await this.#send('clearIntelligenceDefault', { tier });
		this.defaults.update((list) => list.filter((d) => d.tier !== tier));
	}

	/** Asks the daemon to refresh its catalog from the providers, then reloads it. */
	async refreshCatalog(): Promise<void> {
		await this.#send('updateModelCatalog');
		await Promise.all([this.catalog.refetch(), this.providerCatalog.refetch()]);
	}
}

export function createIntelligenceStore(send: Send = defaultSend): IntelligenceStore {
	return new IntelligenceStore(send);
}

export const intelligence = createIntelligenceStore();
