<!--
	Gates a page or section on a `Resource` (stores/resource.svelte.ts): nothing
	while the first fetch runs, the error with a retry when it fails, and
	`loaded` with the value once it lands. A page that opens one row passes
	`missing` for the row that isn't there (a `notFound` error, or a null
	value), with an action that leads back out instead of a retry.
-->
<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import type { ProtocolError } from '$lib/protocol/errors';
	import EmptyState from './EmptyState.svelte';

	/** What this needs of a `Resource`. */
	interface Loadable<V> {
		value: V;
		error: ProtocolError | null;
		loaded: boolean;
		refetch(): Promise<void>;
	}

	interface Props {
		resource: Loadable<T>;
		/** The message for a row that doesn't exist, and the action offered with it. */
		missing?: string;
		missingAction?: string;
		onmissing?: () => void;
		loaded: Snippet<[NonNullable<T>]>;
	}

	let { resource, missing, missingAction, onmissing, loaded }: Props = $props();

	const notFound = $derived(
		resource.error?.code === 'notFound' || (resource.loaded && resource.value === null)
	);
</script>

{#if missing && notFound}
	<EmptyState message={missing} actionLabel={missingAction} onaction={onmissing} />
{:else if resource.error && !resource.loaded}
	<EmptyState
		message={resource.error.message}
		actionLabel="Try again"
		onaction={() => resource.refetch()}
	/>
{:else if resource.loaded && resource.value != null}
	{@render loaded(resource.value)}
{/if}
