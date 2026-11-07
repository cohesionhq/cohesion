<!--
	The import dialog's body: the chosen account's groups in a sidebar ("All"
	first), and beside them a search over the chosen group's records of one kind. Import brings a record into the current
	workspace; its row turns to "Imported" in place, and importing it again
	takes the latest copy.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { flip } from 'svelte/animate';
	import { fade } from 'svelte/transition';
	import { SvelteSet } from 'svelte/reactivity';
	import { ago, nowSeconds } from '$lib/format/dates';
	import { SEARCH_DEBOUNCE_MS } from '$lib/format/search';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ImportableResource, OrganizationUnit } from '$lib/protocol/types';
	import ChooseWorkspace from '$lib/shell/ChooseWorkspace.svelte';
	import { routes } from '$lib/shell/routes';
	import { connectors, type ResourceSearch } from '$lib/stores/connectors.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { list } from '$lib/ui/list';
	import { motion } from '$lib/ui/motion';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { resources } = pageStores();

	const page = pane();

	interface Props {
		kindId: number;
	}

	let { kindId }: Props = $props();

	const ws = $derived(workspaces.currentId);
	const connectorId = $derived(connectors.kind(kindId)?.connector.id);
	/** The accounts to pick from: those of the kind's connector that can import. */
	const accounts = $derived(
		connectors.accounts.value.filter((a) => a.importable && a.connector.id === connectorId)
	);
	/** The chosen account; null until the accounts load. */
	let selectedId = $state<number | null>(null);
	/** The chosen group; null for all of them. */
	let unitId = $state<string | null>(null);
	/** Null while the chosen account's groups load. */
	let units = $state<OrganizationUnit[] | null>(null);
	let query = $state('');
	/** Null until the first search for the chosen account answers. */
	let results = $state<ImportableResource[] | null>(null);
	let error = $state<string | null>(null);
	let generation = 0;
	/** The query last searched, so only typing waits for the debounce. */
	let searched: string | null = null;
	const importing = new SvelteSet<string>();
	const now = nowSeconds();

	$effect(() => {
		untrack(() => void connectors.load());
	});

	const account = $derived(accounts.find((a) => a.id === selectedId));
	const accountOptions = $derived(
		accounts.map((a) => ({ value: String(a.id), label: a.description }))
	);

	// Effects follow the id, so a refetched account list doesn't restart them.
	const accountId = $derived(account?.id);

	// Default to the first account, and again if the chosen one goes away.
	$effect(() => {
		if (!accounts.some((a) => a.id === selectedId)) selectedId = accounts[0]?.id ?? null;
	});

	// A new account has its own groups.
	$effect(() => {
		const id = accountId;
		unitId = null;
		units = null;
		results = null;
		error = null;
		if (id === undefined) return;
		connectors
			.organizationUnits(id)
			.then((found) => {
				if (accountId === id) units = found;
			})
			.catch((e) => {
				if (accountId === id) units = [];
				toasts.error(e);
			});
	});

	$effect(() => {
		const workspace_id = ws;
		const account_id = accountId;
		const organization_unit_id = unitId;
		const q = query.trim();
		if (workspace_id === null || account_id === undefined) return;
		const delay = untrack(() => (searched === null || searched === q ? 0 : SEARCH_DEBOUNCE_MS));
		const timer = setTimeout(
			() =>
				void search({
					account_id,
					workspace_id,
					kind_id: kindId,
					organization_unit_id,
					search: q === '' ? null : q
				}),
			delay
		);
		return () => clearTimeout(timer);
	});

	async function search(args: ResourceSearch) {
		const current = ++generation;
		searched = args.search ?? '';
		try {
			const found = await connectors.searchResources(args);
			if (current !== generation) return;
			results = found;
			error = null;
		} catch (e) {
			if (current === generation) error = toProtocolError(e).message;
		}
	}

	function retry() {
		if (ws === null || !account) return;
		void search({
			account_id: account.id,
			workspace_id: ws,
			kind_id: kindId,
			organization_unit_id: unitId,
			search: query.trim() === '' ? null : query.trim()
		});
	}

	async function importResource(row: ImportableResource) {
		if (ws === null || !account) return;
		importing.add(row.identifier);
		try {
			const resource = await connectors.importResource(account.id, ws, row.identifier);
			resources.imported(resource);
			results =
				results?.map((r) => (r.identifier === row.identifier ? { ...r, imported: true } : r)) ??
				null;
			toasts.show(`Imported “${resource.title}”.`, 'info', {
				href: routes.resource(resource.workspace_id, resource.kind_id, resource.id)
			});
		} catch (e) {
			toasts.error(e);
		} finally {
			importing.delete(row.identifier);
		}
	}

	// Group rows; below 900px each becomes a pill in a wrapping row.
	const choice =
		'flex w-full cursor-pointer items-center rounded-sm border border-transparent px-2 py-1.5 text-left text-fg-muted hover:bg-bg-hover hover:text-fg aria-current:bg-bg-hover aria-current:font-medium aria-current:text-fg max-md:w-auto max-md:rounded-full max-md:border-border max-md:px-3 max-md:py-1';
</script>

{#snippet unitButton(id: string | null, label: string)}
	<li>
		<button
			class={choice}
			type="button"
			aria-current={unitId === id ? 'true' : undefined}
			onclick={() => (unitId = id)}><span class="min-w-0 truncate">{label}</span></button
		>
	</li>
{/snippet}

{#if ws === null}
	<ChooseWorkspace message="Choose a workspace to import into." />
{:else}
	<ResourceState resource={connectors.accounts}>
		{#snippet loaded()}
			{#if accounts.length === 0}
				<EmptyState
					message="No accounts to import from yet"
					actionLabel="Add an account"
					onaction={() => void page.goto(routes.connectorAccounts())}
					compact
				/>
			{:else}
				<!-- A fixed height, so the groups and the results each scroll on their own.
				     Below 900px the groups stack above the results as wrapping pills. -->
				<div
					class="flex h-[min(560px,calc(80dvh-10rem))] gap-5 max-md:h-auto max-md:flex-col max-md:gap-4"
				>
					<nav
						class="flex w-(--sidebar-width) flex-none [scrollbar-width:thin] flex-col gap-3 overflow-y-auto *:flex-none max-md:w-auto max-md:overflow-visible"
						aria-label="Groups"
					>
						{#if accounts.length > 1}
							<Select
								aria-label="Account"
								options={accountOptions}
								bind:value={() => String(selectedId ?? ''), (v) => (selectedId = Number(v))}
							/>
						{/if}
						<ul class="flex flex-col gap-0.5 max-md:flex-row max-md:flex-wrap max-md:gap-1">
							{@render unitButton(null, 'All')}
							{#if units === null}
								<li class="px-2 py-1 text-xs text-fg-subtle">
									<span role="status">Loading…</span>
								</li>
							{:else}
								{#each units as unit (unit.id)}
									{@render unitButton(unit.id, unit.display)}
								{/each}
							{/if}
						</ul>
					</nav>

					<div class="flex min-w-0 flex-1 flex-col gap-4">
						<!-- Stays put while the results scroll under it. -->
						<TextInput
							class="flex-none"
							type="search"
							placeholder={account ? `Search ${account.description}` : 'Search'}
							aria-label="Search {account?.description ?? 'the account'}"
							autocomplete="off"
							bind:value={query}
						/>

						{#if error !== null && results === null}
							<EmptyState message={error} actionLabel="Try again" onaction={retry} compact />
						{:else if results === null}
							<p
								class="flex flex-1 items-center justify-center text-fg-subtle max-md:min-h-60"
								role="status"
							>
								Loading {account?.description ?? 'the account'}…
							</p>
						{:else}
							<!-- A new group or search moves rows in place: new ones fade in, kept ones slide. -->
							<div
								class="min-h-0 flex-1 [scrollbar-width:thin] overflow-y-auto max-md:overflow-visible"
							>
								{#if error !== null}<p class="mb-2 text-xs text-danger" role="alert">
										{error}
									</p>{/if}
								<ul class={list.rows}>
									{#each results as row (row.identifier)}
										{@const busy = importing.has(row.identifier)}
										<li
											class={list.row}
											in:fade={{ duration: motion.duration }}
											animate:flip={{ duration: motion.duration }}
										>
											{#if account}
												<span class="flex self-center">
													<IconTile icon={account.connector.icon} title={account.connector.title} />
												</span>
											{/if}
											<div class={list.main}>
												<span>{row.title}</span>
												{#if row.last_updated !== null}
													<span class="text-xs text-fg-subtle tabular-nums"
														>updated {ago(row.last_updated, now)}</span
													>
												{/if}
											</div>
											<div class={list.actions}>
												{#if row.imported}
													<span
														class="inline-flex h-5 items-center rounded-sm bg-bg-hover px-1.5 text-2xs font-medium text-fg-subtle"
														in:fade={{ duration: motion.duration }}>Imported</span
													>
												{:else}
													<Button
														aria-label="Import {row.title}"
														disabled={busy}
														onclick={() => importResource(row)}
														>{busy ? 'Importing…' : 'Import'}</Button
													>
												{/if}
											</div>
										</li>
									{:else}
										<li class={list.empty}>
											{query.trim() === '' ? 'Nothing to import.' : 'No matches.'}
										</li>
									{/each}
								</ul>
							</div>
						{/if}
					</div>
				</div>
			{/if}
		{/snippet}
	</ResourceState>
{/if}
