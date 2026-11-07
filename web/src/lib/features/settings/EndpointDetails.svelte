<!--
	An endpoint's settings that its row doesn't show (what serves it and its
	prompt cache), behind an info button at the row's end.
-->
<script lang="ts">
	import Info from '@lucide/svelte/icons/info';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { cacheLifetime, type EndpointWithModels } from '$lib/stores/settings/intelligence.svelte';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		endpoint: EndpointWithModels;
		/** The account it signs in with, for an endpoint that signs in with one. */
		account?: string;
	}

	let { endpoint, account }: Props = $props();

	const backend = $derived(endpoint.backend);
	const dialect = $derived(
		options(vocabulary.value.dialects).find((d) => d.value === backend.dialect)?.label ??
			backend.dialect
	);
</script>

<Popover align="end">
	{#snippet trigger(props)}
		<button
			class="inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg"
			type="button"
			aria-label="Details for {endpoint.name}"
			{...props}
		>
			<Info size={16} strokeWidth={1.75} />
		</button>
	{/snippet}
	<dl class="grid max-w-80 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 px-2 py-1.5 text-xs">
		<dt class="text-fg-muted">Provider ID</dt>
		<dd class="font-mono break-all">{backend.provider_identifier}</dd>
		<dt class="text-fg-muted">API format</dt>
		<dd>{dialect}</dd>
		<dt class="text-fg-muted">Base URL</dt>
		<dd class="font-mono break-all">{backend.base_url}</dd>
		{#if account}
			<dt class="text-fg-muted">Account</dt>
			<dd>{account}</dd>
		{/if}
		<dt class="text-fg-muted">Cache</dt>
		<dd>{cacheLifetime(backend.cache_minutes)}</dd>
		<dt class="text-fg-muted">Extended cache</dt>
		<dd>
			{#if backend.long_cache}
				{cacheLifetime(backend.long_cache.minutes)}, written at {backend.long_cache.write_multiple}×
				the cache write price
			{:else}
				None
			{/if}
		</dd>
	</dl>
</Popover>
