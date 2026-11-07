<!--
	Which models to import into an endpoint: a checkbox per model the catalog
	lists for `provider`, filterable, and a field for one it doesn't list.
	Models the endpoint already offers stay checked and can't be unchecked.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { filterCatalog, intelligence } from '$lib/stores/settings/intelligence.svelte';
	import Checkbox from '$lib/ui/Checkbox.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';

	interface Props {
		/** The catalog provider, such as `anthropic`; blank lists nothing. */
		provider: string;
		/** Identifiers of the models the endpoint already offers. */
		imported?: string[];
		/** Identifiers of the checked catalog models. */
		checked?: string[];
		/** A model identifier typed by hand. */
		typed?: string;
		/** Prefix for the controls' ids, unique on the page. */
		id: string;
		/** A note under the list's label. */
		help?: string;
	}

	let {
		provider,
		imported = [],
		checked = $bindable([]),
		typed = $bindable(''),
		id,
		help
	}: Props = $props();

	let query = $state('');

	// Checked models belong to the provider they were listed for.
	$effect(() => {
		const key = provider.trim();
		untrack(() => {
			checked = [];
			if (key !== '') void intelligence.providerCatalog.load(key);
		});
	});
	$effect(() => () => intelligence.providerCatalog.release());

	const catalog = intelligence.providerCatalog;
	const ready = $derived(provider.trim() !== '' && catalog.key === provider.trim());
	const rows = $derived(filterCatalog(catalog.value, query, 'name'));

	function toggle(model_identifier: string, on: boolean) {
		checked = on ? [...checked, model_identifier] : checked.filter((m) => m !== model_identifier);
	}
</script>

<div class="flex flex-col gap-3">
	<div class="flex flex-col gap-1">
		<span class="font-medium">Models</span>
		{#if help}<p class="text-xs text-fg-subtle">{help}</p>{/if}
		{#if !ready}
			<p class="text-xs text-fg-subtle">Enter a provider ID to list its models.</p>
		{:else if catalog.error && !catalog.loaded}
			<p class="text-xs text-danger">{catalog.error.message}</p>
		{:else if catalog.loaded && catalog.value.length === 0}
			<p class="text-xs text-fg-subtle">No models for this provider yet.</p>
		{:else if catalog.loaded}
			<TextInput
				bind:value={query}
				type="search"
				placeholder="Filter models"
				aria-label="Filter models"
			/>
			<ul
				class="max-h-56 overflow-y-auto rounded-sm border border-border"
				aria-label="Catalog models"
			>
				{#each rows as model (model.id)}
					{@const offered = imported.includes(model.model_identifier)}
					<li class="border-b border-border px-2 py-1.5 last:border-b-0">
						<Checkbox
							checked={offered || checked.includes(model.model_identifier)}
							disabled={offered}
							onchange={(event) => toggle(model.model_identifier, event.currentTarget.checked)}
						>
							{model.model_name}
							<span class="font-mono text-2xs wrap-anywhere text-fg-subtle"
								>{model.model_identifier}</span
							>
						</Checkbox>
					</li>
				{:else}
					<li class="px-2 py-1.5 text-fg-muted">No matches.</li>
				{/each}
			</ul>
		{/if}
	</div>
	<Field
		label="Other model (optional)"
		for="{id}-typed"
		help="Enter the ID of a model the catalog doesn’t list."
	>
		<TextInput id="{id}-typed" bind:value={typed} autocomplete="off" spellcheck="false" />
	</Field>
</div>
