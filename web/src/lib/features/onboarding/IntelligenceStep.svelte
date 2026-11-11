<!--
	The flow's second page: connects a model provider. Each built-in provider
	is a tile; picking one asks for its API key, and the provider's
	recommended model comes with the endpoint. A custom endpoint is left to
	Settings.
-->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { toProtocolError } from '$lib/protocol/errors';
	import {
		emptyEndpointForm,
		intelligence,
		presets,
		providerLabel,
		type PresetName
	} from '$lib/stores/settings/intelligence.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		onback: () => void;
		onnext: () => void;
	}

	let { onback, onnext }: Props = $props();

	const id = $props.id();
	const providers = presets().filter((p) => p.value !== 'custom');
	let preset = $state<PresetName | null>(null);
	let apiKey = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	const endpoints = $derived(intelligence.endpoints.value);
	const connected = $derived(endpoints.length > 0);
	const chosen = $derived(providers.find((p) => p.value === preset) ?? null);

	function choose(value: PresetName) {
		preset = value;
		apiKey = '';
		error = null;
	}

	async function connect(event: SubmitEvent) {
		event.preventDefault();
		if (!preset) return;
		busy = true;
		error = null;
		try {
			await intelligence.createEndpoint({ ...emptyEndpointForm(), preset, api_key: apiKey });
			preset = null;
			apiKey = '';
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<StepPage
	eyebrow="Step 2 of 3"
	title={connected ? 'Brains, plugged in.' : 'Plug in some brains.'}
	description={connected
		? 'Agents can think with these models now. Add another provider, or carry on.'
		: 'Agents think with models from a provider you choose. Pick one and paste its API key. Cohesion sets up its recommended model for you.'}
	{onback}
	onskip={connected ? undefined : onnext}
>
	{#if connected}
		<ul class="flex flex-wrap justify-center gap-2" aria-label="Connected">
			{#each endpoints as endpoint (endpoint.id)}
				<li
					class="flex items-center gap-2 rounded-full border border-border py-1 pr-3 pl-1 text-xs"
				>
					<span class="grid size-5 place-items-center rounded-full bg-ok text-on-fill">
						<Check size={12} strokeWidth={3} aria-hidden="true" />
					</span>
					<span class="font-medium">{endpoint.name}</span>
					{#if providerLabel(endpoint.backend.provider_identifier) !== endpoint.name}
						<span class="text-fg-subtle">{providerLabel(endpoint.backend.provider_identifier)}</span
						>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	<div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Provider">
		{#each providers as provider (provider.value)}
			{@const on = preset === provider.value}
			<button
				type="button"
				role="radio"
				aria-checked={on}
				class={[
					'flex cursor-pointer items-center gap-2.5 rounded-md border px-2.5 py-1.5 text-xs font-medium transition-colors focus-visible:shadow-focus',
					on
						? 'border-accent bg-accent-soft text-fg'
						: 'border-border text-fg-muted hover:bg-bg-hover hover:text-fg'
				]}
				onclick={() => choose(provider.value)}
			>
				<IconTile icon={null} title={provider.label} size={28} />
				{provider.label}
			</button>
		{/each}
	</div>

	{#if chosen}
		<form id="{id}-form" onsubmit={connect}>
			<Field label="{chosen.label} API key" for="{id}-key" help="Never shown again." {error}>
				<TextInput
					id="{id}-key"
					type="password"
					bind:value={apiKey}
					autocomplete="off"
					autofocus
					required
				/>
			</Field>
		</form>
	{/if}

	{#snippet action()}
		{#if chosen}
			<Button
				variant="primary"
				type="submit"
				form="{id}-form"
				disabled={busy || apiKey.trim() === ''}
			>
				{busy ? 'Connecting…' : `Connect ${chosen.label}`}
			</Button>
		{:else}
			<Button variant="primary" onclick={onnext} disabled={!connected}>Continue</Button>
		{/if}
	{/snippet}
</StepPage>
