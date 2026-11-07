<!--
	One kind's resources in a workspace, filtered by account and title and
	sorted by title, last update or a metadata key its connector declares,
	on the page its sidebar link opens: refreshing them, and importing more.
	The account filter and each row's account show only once the kind's
	resources come from more than one account.
-->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { untrack } from 'svelte';
	import { flip } from 'svelte/animate';
	import { fade } from 'svelte/transition';
	import { nowSeconds, relativeTime } from '$lib/format/dates';
	import { routes } from '$lib/shell/routes';
	import type { MetadataKey, ResourceSort } from '$lib/protocol/types';
	import { rememberOrder, rememberedOrder, type ResourceOrder } from '$lib/stores/resources.svelte';
	import { connectors } from '$lib/stores/connectors.svelte';
	import ImportDialog from './ImportDialog.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { motion } from '$lib/ui/motion';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import SortMenu, { type SortOption } from '$lib/ui/SortMenu.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { resources } = pageStores();

	interface Props {
		ws: number;
		kindId: number;
	}

	let { ws, kindId }: Props = $props();

	const EARLIEST: [string, string] = ['Earliest first', 'Latest first'];
	const ALPHABETICAL: [string, string] = ['A to Z', 'Z to A'];
	/** Each metadata key a list sorts by, with its label and directions; a URL doesn't. */
	const METADATA_SORTS: Record<Exclude<MetadataKey, 'url'>, [string, [string, string]]> = {
		dateCreated: ['Date created', EARLIEST],
		dateModified: ['Date modified', EARLIEST],
		dateDue: ['Due date', EARLIEST],
		dateStart: ['Start date', EARLIEST],
		dateEnd: ['End date', EARLIEST],
		author: ['Author', ALPHABETICAL],
		location: ['Location', ALPHABETICAL],
		folder: ['Folder', ALPHABETICAL],
		category: ['Category', ALPHABETICAL],
		keywords: ['Tags', ALPHABETICAL],
		status: ['Status', ALPHABETICAL]
	};

	const entry = $derived(connectors.kind(kindId));
	const title = $derived(entry?.kind.plural ?? 'Resources');
	/** The workspace's resources of `kindId`. */
	const listed = $derived(resources.list.value.filter((r) => r.kind_id === kindId));
	/** The chosen account's id, or null for all of them. */
	let accountId = $state<number | null>(null);
	let query = $state('');
	/** The order picked on this page, until the kind changes; its remembered one before. */
	let picked = $state<ResourceOrder | null>(null);
	let refreshing = $state(false);
	let importing = $state(false);
	const now = nowSeconds();

	$effect(() => {
		untrack(() => void connectors.load());
	});

	/** Title, last update, then the keys the kind's connector declares. */
	const sorts = $derived<SortOption<ResourceSort>[]>([
		{ value: 'title', label: 'Title', directions: ALPHABETICAL },
		{
			value: 'lastUpdated',
			label: 'Last updated',
			directions: ['Oldest first', 'Newest first'],
			descendingFirst: true
		},
		...(entry?.connector.metadata_keys ?? []).flatMap((key) =>
			key === 'url'
				? []
				: [{ value: key, label: METADATA_SORTS[key][0], directions: METADATA_SORTS[key][1] }]
		)
	]);
	const order = $derived(
		picked ??
			rememberedOrder(
				kindId,
				sorts.map((s) => s.value)
			)
	);

	function setOrder(next: ResourceOrder) {
		picked = next;
		rememberOrder(kindId, next);
	}

	$effect(() => {
		void ws;
		void kindId;
		accountId = null;
		query = '';
		picked = null;
	});

	$effect(() => {
		const id = ws;
		const by = order;
		untrack(() => void resources.load(id, by));
		return () => resources.list.release();
	});

	/** Each account the listed resources came from, once, by its description. */
	const sources = $derived([
		...new Map(
			listed.map((r) => [
				r.account_id,
				connectors.account(r.account_id)?.description ?? String(r.account_id)
			])
		)
	]);
	const accountLabel = $derived(sources.find(([id]) => id === accountId)?.[1] ?? 'All');
	const accountItems = $derived<MenuItem[]>(
		[[null, 'All'] as const, ...sources].map(([id, label]) => ({
			id: String(id),
			label,
			checked: id === accountId,
			onSelect: () => (accountId = id)
		}))
	);

	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return listed.filter(
			(a) =>
				(accountId === null || a.account_id === accountId) &&
				(q === '' || a.title.toLowerCase().includes(q))
		);
	});

	async function refresh() {
		refreshing = true;
		try {
			const { checked, updated, failed } = await resources.refresh(ws);
			toasts.show(
				updated === 0 && failed === 0
					? 'Everything is up to date.'
					: `Updated ${updated} of ${checked}${failed > 0 ? `; ${failed} failed` : ''}.`,
				failed > 0 ? 'error' : 'info'
			);
		} catch (error) {
			toasts.error(error);
		} finally {
			refreshing = false;
		}
	}
</script>

<PageTitle {title} />

<!-- Full height, so the empty state can centre in what's left under the header. -->
<div class="flex min-h-full flex-col px-(--page-inset)">
	<PageHeader {title}>
		{#snippet actions()}
			<Button disabled={refreshing} onclick={refresh}
				>{refreshing ? 'Refreshing…' : 'Refresh'}</Button
			>
			<Button variant="primary" onclick={() => (importing = true)}>Import…</Button>
		{/snippet}
	</PageHeader>

	<ResourceState resource={resources.list}>
		{#snippet loaded()}
			{#if listed.length === 0}
				<EmptyState message="No {title.toLowerCase()} yet" centered />
			{:else}
				<div class="mb-5 flex flex-wrap gap-2">
					{#if sources.length > 1}
						<Menu items={accountItems} align="start">
							{#snippet trigger(props)}
								<Button
									{...props}
									class="border-border-strong!"
									aria-label="Account: {accountLabel}"
								>
									{accountLabel}
									<ChevronDown size={14} strokeWidth={1.75} aria-hidden="true" />
								</Button>
							{/snippet}
						</Menu>
					{/if}
					<div class="max-w-80 min-w-48 flex-1">
						<TextInput
							bind:value={query}
							type="search"
							placeholder="Filter by title"
							aria-label="Filter resources by title"
							autocomplete="off"
						/>
					</div>
					<div class="ml-auto self-center">
						<SortMenu {sorts} {order} noun={title.toLowerCase()} onchange={setOrder} />
					</div>
				</div>

				{#if shown.length === 0}
					<p class="text-fg-muted">No matches.</p>
				{:else}
					<ul>
						{#each shown as resource (resource.id)}
							{@const account = connectors.account(resource.account_id)}
							<!-- Filtering fades rows in and slides the rest into place; filtered-out rows go at once. -->
							<li
								class="border-border not-first:border-t"
								in:fade={{ duration: motion.duration }}
								animate:flip={{ duration: motion.duration }}
							>
								<a
									class="group flex items-center gap-3 py-2 text-fg hover:no-underline"
									href={routes.resource(ws, kindId, resource.id)}
								>
									{#if account}
										<IconTile icon={account.connector.icon} title={account.connector.title} />
									{/if}
									<span class="flex min-w-0 flex-1 items-baseline gap-2">
										<span class="min-w-0 truncate transition-colors group-hover:text-accent"
											>{resource.title}</span
										>
										{#if sources.length > 1 && account}
											<span class="flex-none text-xs text-fg-muted">{account.description}</span>
										{/if}
									</span>
									{#if resource.last_updated > 0}
										<span class="flex-none text-xs text-fg-subtle tabular-nums"
											>{relativeTime(resource.last_updated, now)}</span
										>
									{/if}
								</a>
							</li>
						{/each}
					</ul>
				{/if}
			{/if}
		{/snippet}
	</ResourceState>
</div>

<ImportDialog bind:open={importing} {kindId} />
