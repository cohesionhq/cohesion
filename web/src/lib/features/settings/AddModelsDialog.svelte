<!-- Imports more models into an existing endpoint, from its provider's catalog or by identifier. -->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import {
		intelligence,
		modelsToImport,
		type EndpointWithModels
	} from '$lib/stores/settings/intelligence.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import ModelChecklist from './ModelChecklist.svelte';

	interface Props {
		/** The endpoint to add to; the dialog is open while it is set. */
		endpoint: EndpointWithModels | null;
		/** Clears `endpoint`, once the dialog has faded out. */
		onclose: () => void;
	}

	let { endpoint, onclose }: Props = $props();

	let checked = $state<string[]>([]);
	let typed = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);
	/** Set from closing until the fade ends, while `endpoint` is still set. */
	let closing = $state(false);

	// Each open starts from nothing picked.
	$effect(() => {
		if (endpoint) {
			checked = [];
			typed = '';
			error = null;
		}
	});

	// Read from the store, so models imported before a failure count as offered.
	const offered = $derived(
		intelligence.endpoints.value.find((e) => e.id === endpoint?.id)?.models ?? []
	);
	const imported = $derived(offered.map((m) => m.model_identifier));
	const picked = $derived(modelsToImport(checked, typed, imported));

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!endpoint) return;
		busy = true;
		error = null;
		try {
			await intelligence.importModels(endpoint.id, picked);
			closing = true;
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open={
		() => endpoint !== null && !closing,
		(value) => {
			if (!value) closing = true;
		}
	}
	title="Add models to {endpoint?.name ?? 'endpoint'}"
	onclosed={() => {
		closing = false;
		onclose();
	}}
>
	{#if endpoint}
		<form id="models-form" class="flex flex-col gap-3" onsubmit={submit}>
			<ModelChecklist
				id="endpoint-{endpoint.id}-models"
				provider={endpoint.backend.provider_identifier}
				{imported}
				bind:checked
				bind:typed
			/>
			{#if error}<p class="text-xs text-danger" role="alert">{error}</p>{/if}
		</form>
	{/if}
	{#snippet footer()}
		<Button onclick={() => (closing = true)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="models-form"
			disabled={busy || picked.length === 0}
		>
			{busy ? 'Adding…' : 'Add models'}
		</Button>
	{/snippet}
</Dialog>
