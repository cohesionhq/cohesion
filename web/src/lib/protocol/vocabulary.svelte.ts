// How the daemon words and orders its kinds: what each permission, hook,
// priority, size, actor session status, agent setting, pairing opener,
// security event and linked item is called, in the order the UI lists them.
// Every `hello` carries it, so the page keeps no copy of its own; mock mode
// and tests read the same words from `vocabulary.json`, which
// `npm run generate` writes from the daemon.

import type {
	ActorSessionStatus,
	PropertyDataType,
	ActorSessionStop,
	EmbedKind,
	ExtensionPermissionKind,
	HookKind,
	HookWording,
	SecurityEventKind,
	TaskPriority,
	TaskSize,
	Vocabulary as Words
} from './types';

/** A kind with a title and a one-line description, as `Vocabulary` lists one. */
export type Described<K> = { kind: K; title: string; description: string };

const EMPTY: Words = {
	permissions: [],
	hooks: [],
	task_priorities: [],
	version_changes: [],
	task_sizes: [],
	actor_session_statuses: [],
	actor_session_stops: [],
	agent_colors: [],
	list_symbols: [],
	agent_personalities: [],
	agent_verbosities: [],
	tiers: [],
	spend_periods: [],
	providers: [],
	dialects: [],
	agent_reactions: [],
	pairing_openers: [],
	security_events: [],
	embed_kinds: [],
	property_data_types: [],
	formula_update_modes: []
};

class Vocabulary {
	/** The daemon's latest words; empty until the first `hello`. */
	value = $state<Words>(EMPTY);

	#permissions = $derived(byKind(this.value.permissions));
	#hooks = $derived(byKind(this.value.hooks));
	#priorities = $derived(byKind(this.value.task_priorities));
	#sizes = $derived(byKind(this.value.task_sizes));
	#actorSessionStatuses = $derived(byKind(this.value.actor_session_statuses));
	#actorSessionStops = $derived(byKind(this.value.actor_session_stops));
	#securityEvents = $derived(byKind(this.value.security_events));
	#embedKinds = $derived(byKind(this.value.embed_kinds));
	#propertyDataTypes = $derived(byKind(this.value.property_data_types));

	permission(kind: ExtensionPermissionKind): Described<ExtensionPermissionKind> {
		return this.#permissions.get(kind)!;
	}

	hook(kind: HookKind): HookWording {
		return this.#hooks.get(kind)!;
	}

	priority(kind: TaskPriority): string {
		return this.#priorities.get(kind)!.label;
	}

	size(kind: TaskSize): string {
		return this.#sizes.get(kind)!.label;
	}

	actorSessionStatus(kind: ActorSessionStatus): string {
		return this.#actorSessionStatuses.get(kind)!.label;
	}

	/** How an actor session stopped on its own, as a clause after its title. */
	actorSessionStop(kind: ActorSessionStop): string {
		return this.#actorSessionStops.get(kind)!.label;
	}

	securityEvent(kind: SecurityEventKind): string {
		return this.#securityEvents.get(kind)!.label;
	}

	/** What an item a `cohesion://` link names is called: `Task`, `Journal entry`. */
	embedKind(kind: EmbedKind): string {
		return this.#embedKinds.get(kind)!.label;
	}

	/** What a table column's type of value is called: `Whole number`, `Currency`. */
	propertyDataType(kind: PropertyDataType): string {
		return this.#propertyDataTypes.get(kind)!.label;
	}

	/** What to call the provider whose catalog id is `identifier`, if it has a preset. */
	provider(identifier: string): string | undefined {
		return this.value.providers.find((p) => p.provider_identifier === identifier)?.label;
	}
}

function byKind<K, T extends { kind: K }>(list: T[]): Map<K, T> {
	return new Map(list.map((item) => [item.kind, item]));
}

/** A list of kinds as `Select` options. */
export function options<K>(list: { kind: K; label: string }[]): { value: K; label: string }[] {
	return list.map(({ kind, label }) => ({ value: kind, label }));
}

export const vocabulary = new Vocabulary();
