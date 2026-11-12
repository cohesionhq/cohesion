<script lang="ts">
	import { untrack } from 'svelte';
	import type { EndpointModel } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import {
		intelligence,
		modelGroups,
		providerLabel,
		signsInWithAccount,
		type AccountEndpoint,
		type EndpointWithModels
	} from '$lib/stores/settings/intelligence.svelte';
	import { connectors } from '$lib/stores/connectors.svelte';
	import { spend } from '$lib/stores/settings/spend.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { list } from '$lib/ui/list';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import AddEndpointDialog from './AddEndpointDialog.svelte';
	import AddModelsDialog from './AddModelsDialog.svelte';
	import CatalogTable from './CatalogTable.svelte';
	import EndpointDetails from './EndpointDetails.svelte';
	import DefaultRow from './DefaultRow.svelte';
	import Section from './Section.svelte';
	import SpendLimitRow from './SpendLimitRow.svelte';

	let addingEndpoint = $state(false);
	let addingModels = $state<EndpointWithModels | null>(null);
	let deletingEndpoint = $state<EndpointWithModels | null>(null);
	let removingAccount = $state<AccountEndpoint | null>(null);
	let removingModel = $state<{ model: EndpointModel; endpoint: EndpointWithModels } | null>(null);

	let catalogQuery = $state('');
	let catalogEndpoint = $state('');
	let refreshing = $state(false);

	$effect(() => {
		untrack(() => void Promise.all([intelligence.load(), connectors.load(), spend.load()]));
		return () => {
			intelligence.release();
			connectors.accounts.release();
			spend.periods.release();
		};
	});

	/** What the account `endpoint` signs in with is called, such as its email. */
	function accountName(endpoint: AccountEndpoint): string {
		return connectors.account(endpoint.backend.account_id)?.description ?? 'account';
	}

	/**
	 * Removes the account `endpoint` signs in with, which takes the endpoint
	 * and the defaults using its models with it.
	 */
	async function removeAccount(endpoint: AccountEndpoint) {
		await connectors.remove(endpoint.backend.account_id);
		await Promise.all([intelligence.endpoints.refetch(), intelligence.defaults.refetch()]);
	}

	const endpoints = $derived(intelligence.endpoints.value);

	// Only the endpoint filter should re-run this; `load` reads the resource's own state.
	$effect(() => {
		const provider =
			endpoints.find((e) => String(e.id) === catalogEndpoint)?.backend.provider_identifier ?? null;
		untrack(() => void intelligence.catalog.load(provider));
		return () => intelligence.catalog.release();
	});

	// A closed select shows only the option, so it names the endpoint too:
	// two endpoints may offer a model of the same name.
	const groups = $derived(modelGroups(endpoints));
	const endpointFilter = $derived([
		{ value: '', label: 'All endpoints' },
		...endpoints.map((e) => ({ value: String(e.id), label: e.name }))
	]);

	const firstError = $derived(
		[intelligence.endpoints, intelligence.defaults].find((r) => r.error && !r.loaded)?.error ?? null
	);
	const loaded = $derived(intelligence.endpoints.loaded && intelligence.defaults.loaded);

	async function refreshCatalog() {
		refreshing = true;
		try {
			await intelligence.refreshCatalog();
		} catch (error) {
			toasts.error(error);
		} finally {
			refreshing = false;
		}
	}
</script>

{#if firstError}
	<EmptyState
		message={firstError.message}
		actionLabel="Try again"
		onaction={() => {
			void intelligence.endpoints.refetch();
			void intelligence.defaults.refetch();
		}}
	/>
{:else if loaded}
	<Section
		title="Endpoints"
		description="Cohesion reaches models through these endpoints. Each endpoint lists the models it offers."
	>
		{#snippet actions()}
			<Button variant="primary" onclick={() => (addingEndpoint = true)}>Add endpoint…</Button>
		{/snippet}
		<ul class={list.rows}>
			{#each endpoints as endpoint (endpoint.id)}
				{@const provider = providerLabel(endpoint.backend.provider_identifier)}
				{@const usageUrl = endpoint.backend.usage_url}
				<li class={list.row}>
					<div class={list.main}>
						<span class="font-medium">{endpoint.name}</span>
						{#if signsInWithAccount(endpoint)}
							<span class="text-fg-muted">{provider} ({accountName(endpoint)})</span>
						{:else if provider !== endpoint.name}
							<span class="text-fg-muted">{provider}</span>
						{/if}
					</div>
					<div class={list.actions}>
						<EndpointDetails
							{endpoint}
							account={signsInWithAccount(endpoint) ? accountName(endpoint) : undefined}
						/>
						<RowMenu
							label="More for {endpoint.name}"
							items={[
								{ label: 'Add models…', onSelect: () => (addingModels = endpoint) },
								...(usageUrl
									? [
											{
												label: 'Usage',
												onSelect: () => window.open(usageUrl, '_blank', 'noopener,noreferrer')
											}
										]
									: []),
								signsInWithAccount(endpoint)
									? {
											label: 'Remove account…',
											danger: true,
											separated: true,
											onSelect: () => (removingAccount = endpoint)
										}
									: {
											label: 'Delete…',
											danger: true,
											separated: true,
											onSelect: () => (deletingEndpoint = endpoint)
										}
							]}
						/>
					</div>
					<ul class="basis-full pl-4" aria-label="{endpoint.name} models">
						{#each endpoint.models as model (model.id)}
							<li class="flex min-h-8 items-center gap-x-2 border-t border-border">
								<div class={list.main}>
									<span>{model.model_name}</span>
									<span class="font-mono text-xs text-fg-subtle">{model.model_identifier}</span>
								</div>
								<RowMenu
									label="More for {model.model_name}"
									items={[
										{
											label: 'Remove…',
											danger: true,
											onSelect: () => (removingModel = { model, endpoint })
										}
									]}
								/>
							</li>
						{:else}
							<li class="border-t border-border py-1.5 text-xs text-fg-muted">
								No models yet. Add some to pick them below.
							</li>
						{/each}
					</ul>
				</li>
			{:else}
				<li>
					<EmptyState compact message="No endpoints yet" detail="Add one to start using models." />
				</li>
			{/each}
		</ul>
	</Section>

	<Section
		title="Defaults"
		description="Each kind of work uses its default model when no model is named."
	>
		<ul class={list.rows}>
			{#each vocabulary.value.tiers as { kind, title, description } (kind)}
				<DefaultRow
					tier={kind}
					label={title}
					help={description}
					{groups}
					current={intelligence.defaults.value.find((d) => d.tier === kind)}
				/>
			{/each}
		</ul>
	</Section>

	<Section
		title="Spend limits"
		description="Limit how much all model calls together can cost, counted back from now. Once a limit is reached, agents call models only to answer your messages."
	>
		<ResourceState resource={spend.periods}>
			{#snippet loaded(periods)}
				<ul class={list.rows}>
					{#each vocabulary.value.spend_periods as { kind, title, description } (kind)}
						{@const current = periods.find((p) => p.period === kind)}
						{#if current}
							<SpendLimitRow {current} label={title} help={description} />
						{/if}
					{/each}
				</ul>
			{/snippet}
		</ResourceState>
	</Section>

	<Section
		title="Catalog"
		description="This lists every model your endpoints’ providers offer. Prices are per million tokens."
	>
		{#snippet actions()}
			<Button onclick={refreshCatalog} disabled={refreshing || endpoints.length === 0}>
				{refreshing ? 'Refreshing…' : 'Refresh catalog'}
			</Button>
		{/snippet}
		{#if endpoints.length === 0}
			<p class={list.empty}>Add an endpoint to see its models.</p>
		{:else}
			<div class="mb-2 flex flex-wrap gap-2">
				<TextInput
					bind:value={catalogQuery}
					class="w-auto flex-[1_1_220px]"
					type="search"
					placeholder="Filter models"
					aria-label="Filter models"
				/>
				<Select bind:value={catalogEndpoint} options={endpointFilter} aria-label="Endpoint" />
			</div>
			{#if intelligence.catalog.error && !intelligence.catalog.loaded}
				<p class={list.error}>{intelligence.catalog.error.message}</p>
			{:else if intelligence.catalog.loaded}
				<CatalogTable models={intelligence.catalog.value} query={catalogQuery} />
			{/if}
		{/if}
	</Section>
{/if}

<AddEndpointDialog bind:open={addingEndpoint} />

<AddModelsDialog endpoint={addingModels} onclose={() => (addingModels = null)} />

<ConfirmDialog
	subject={deletingEndpoint}
	onclose={() => (deletingEndpoint = null)}
	title="Delete {deletingEndpoint?.name ?? 'endpoint'}?"
	description="This endpoint’s models will also be deleted. Any default that uses one of them will be cleared."
	onconfirm={(endpoint) => intelligence.deleteEndpoint(endpoint.id)}
/>

<ConfirmDialog
	subject={removingAccount}
	onclose={() => (removingAccount = null)}
	title="Remove {removingAccount ? accountName(removingAccount) : 'account'}?"
	description="{removingAccount?.name ??
		'The account’s endpoint'} and its models will also be removed. Any default that uses one of them will be cleared."
	confirmLabel="Remove"
	onconfirm={removeAccount}
/>

<ConfirmDialog
	subject={removingModel}
	onclose={() => (removingModel = null)}
	title="Remove {removingModel?.model.model_name ?? 'model'} from {removingModel?.endpoint.name ??
		'endpoint'}?"
	description="Any default that uses this model will be cleared."
	confirmLabel="Remove"
	onconfirm={({ model }) => intelligence.removeModel(model.id)}
/>
