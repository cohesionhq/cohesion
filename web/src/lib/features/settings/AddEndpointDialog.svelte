<!--
	Adds an endpoint: its provider and what that provider needs (a custom one's identifier, backend
	and URL; an API key), the models to import, and under More options the fields nobody has to
	fill: a name, a usage page and a custom endpoint's prompt cache.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import {
		emptyEndpointForm,
		formProvider,
		intelligence,
		modelsToImport,
		presets,
		requiresApiKey,
		type EndpointForm
	} from '$lib/stores/settings/intelligence.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Disclosure from '$lib/ui/Disclosure.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import ModelChecklist from './ModelChecklist.svelte';

	interface Props {
		open?: boolean;
	}

	let { open = $bindable(false) }: Props = $props();

	let form = $state<EndpointForm>(emptyEndpointForm());
	let checked = $state<string[]>([]);
	let typed = $state('');
	let fieldErrors = $state<Record<string, string>>({});
	let formError = $state<string | null>(null);
	let busy = $state(false);
	/** Whether More options is open. */
	let more = $state(false);
	/** The fields under More options, so an error on one opens it. */
	const optionalFields = ['name', 'usage_url', 'cache_minutes'];

	const custom = $derived(form.preset === 'custom');
	const provider = $derived(formProvider(form));
	const keyRequired = $derived(requiresApiKey(form.preset));
	// A long cache takes its lifetime and its price together, or neither.
	const longCacheHalf = $derived(
		custom &&
			(form.long_cache_minutes.trim() === '') !== (form.long_cache_write_multiple.trim() === '')
	);
	const ready = $derived(
		(!keyRequired || form.api_key.trim() !== '') &&
			(!custom || (form.provider_identifier.trim() !== '' && form.base_url.trim() !== '')) &&
			!longCacheHalf
	);

	// Each open starts from a blank form.
	$effect(() => {
		if (open) {
			form = emptyEndpointForm();
			checked = [];
			typed = '';
			fieldErrors = {};
			formError = null;
			more = false;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		fieldErrors = {};
		formError = null;
		let created;
		try {
			created = await intelligence.createEndpoint(form);
		} catch (error) {
			const e = toProtocolError(error);
			if (e.field) {
				fieldErrors = { [e.field]: e.message };
				if (optionalFields.includes(e.field)) more = true;
			} else formError = e.message;
			busy = false;
			return;
		}
		// The endpoint exists now, so a model that fails to import is reported, not retried here.
		try {
			const offered = created.models.map((m) => m.model_identifier);
			await intelligence.importModels(created.id, modelsToImport(checked, typed, offered));
		} catch (error) {
			toasts.error(error);
		}
		busy = false;
		open = false;
	}
</script>

<Dialog bind:open title="Add endpoint">
	<form id="endpoint-form" class="flex flex-col gap-3" onsubmit={submit}>
		<Field label="Provider" for="endpoint-preset" error={fieldErrors.preset}>
			<Select id="endpoint-preset" bind:value={form.preset} options={presets()} />
		</Field>
		{#if custom}
			<Field label="Provider ID" for="endpoint-provider" error={fieldErrors.provider_identifier}>
				<TextInput
					id="endpoint-provider"
					bind:value={form.provider_identifier}
					autocomplete="off"
					spellcheck="false"
					required
				/>
			</Field>
			<Field label="API format" for="endpoint-dialect" error={fieldErrors.dialect}>
				<Select
					id="endpoint-dialect"
					bind:value={form.dialect}
					options={options(vocabulary.value.dialects)}
				/>
			</Field>
			<Field label="Base URL" for="endpoint-url" error={fieldErrors.base_url}>
				<TextInput
					id="endpoint-url"
					type="url"
					bind:value={form.base_url}
					placeholder="https://…/v1"
					autocomplete="off"
					required
				/>
			</Field>
		{/if}
		<Field
			label={keyRequired ? 'API key' : 'API key (optional)'}
			for="endpoint-key"
			help="Never shown again."
			error={fieldErrors.api_key}
		>
			<TextInput
				id="endpoint-key"
				type="password"
				bind:value={form.api_key}
				autocomplete="off"
				required={keyRequired}
			/>
		</Field>
		<ModelChecklist
			id="endpoint-models"
			{provider}
			help={custom ? undefined : 'The provider’s recommended model is imported too.'}
			bind:checked
			bind:typed
		/>
		<Disclosure label="More options" bind:open={more} class="gap-3 pt-3">
			<Field
				label="Name (optional)"
				for="endpoint-name"
				help="Leave it blank to name it after the provider."
				error={fieldErrors.name}
			>
				<TextInput
					id="endpoint-name"
					bind:value={form.name}
					autocomplete="off"
					spellcheck="false"
				/>
			</Field>
			<Field
				label="Usage URL (optional)"
				for="endpoint-usage"
				help={custom
					? 'Where the provider shows what this endpoint has used.'
					: 'Leave it blank to use the provider’s usage page.'}
				error={fieldErrors.usage_url}
			>
				<TextInput
					id="endpoint-usage"
					type="url"
					bind:value={form.usage_url}
					placeholder="https://…"
					autocomplete="off"
				/>
			</Field>
			<!-- A preset's provider comes with its cache, so only a custom endpoint describes one. -->
			{#if custom}
				<fieldset class="flex flex-col gap-1">
					<legend class="mb-1 font-medium">Prompt cache (optional)</legend>
					<div class="grid grid-cols-3 gap-2">
						<div class="flex flex-col gap-1">
							<label class="text-xs text-fg-muted" for="endpoint-cache">Minutes</label>
							<TextInput
								id="endpoint-cache"
								bind:value={form.cache_minutes}
								inputmode="numeric"
								pattern="[1-9][0-9]*"
								autocomplete="off"
							/>
						</div>
						<div class="flex flex-col gap-1">
							<label class="text-xs text-fg-muted" for="endpoint-long-cache"
								>Extended cache duration</label
							>
							<TextInput
								id="endpoint-long-cache"
								bind:value={form.long_cache_minutes}
								inputmode="numeric"
								pattern="[1-9][0-9]*"
								autocomplete="off"
							/>
						</div>
						<div class="flex flex-col gap-1">
							<label class="text-xs text-fg-muted" for="endpoint-long-cache-price">
								Extended cache write price
							</label>
							<TextInput
								id="endpoint-long-cache-price"
								bind:value={form.long_cache_write_multiple}
								inputmode="decimal"
								pattern="[0-9]*\.?[0-9]+"
								autocomplete="off"
							/>
						</div>
					</div>
					{#if fieldErrors.cache_minutes}
						<p class="text-xs text-danger">{fieldErrors.cache_minutes}</p>
					{:else}
						<p class="text-xs text-fg-subtle">
							Enter how many minutes the provider keeps a prompt cached. Agents ask for the extended
							cache. The extended cache’s write price is a multiple of the cache write price, such
							as 1.6. Leave these blank for 5 minutes and no extended cache.
						</p>
					{/if}
				</fieldset>
			{/if}
		</Disclosure>
		{#if formError}<p class="text-xs text-danger" role="alert">{formError}</p>{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="endpoint-form" disabled={busy || !ready}>
			{busy ? 'Adding…' : 'Add endpoint'}
		</Button>
	{/snippet}
</Dialog>
