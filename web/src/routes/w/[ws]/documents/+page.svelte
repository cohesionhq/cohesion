<script lang="ts">
	import SearchField from '$lib/ui/SearchField.svelte';
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import { localMidnight, nowSeconds, relativeTime } from '$lib/format/dates';
	import { SEARCH_DEBOUNCE_MS } from '$lib/format/search';
	import type { Document } from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { routes } from '$lib/shell/routes';
	import { SERVER_FILTER_THRESHOLD, groupByRecency } from '$lib/stores/documents.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { documents } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	let query = $state('');
	let creating = $state(false);
	/** Server-side matches for `query` when the list is too long to filter here. */
	let found = $state<Document[] | null>(null);

	$effect(() => {
		const id = ws;
		untrack(() => void documents.list.load(id));
		query = '';
	});

	const remote = $derived(documents.list.value.length > SERVER_FILTER_THRESHOLD);

	$effect(() => {
		const q = query.trim();
		const id = ws;
		found = null;
		if (!remote || q === '') return;
		const timer = setTimeout(() => {
			documents
				.search(id, q)
				.then((docs) => {
					if (q === query.trim() && id === ws) found = docs;
				})
				.catch((error) => toasts.error(error));
		}, SEARCH_DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (q === '') return documents.list.value;
		if (remote) return found ?? [];
		return documents.list.value.filter((d) => d.title.toLowerCase().includes(q));
	});

	const now = nowSeconds();
	const groups = $derived(groupByRecency(shown, localMidnight()));

	async function create() {
		creating = true;
		try {
			const doc = await documents.create(ws);
			void page.goto(routes.document(ws, doc.id));
		} catch (error) {
			toasts.error(error);
		} finally {
			creating = false;
		}
	}
</script>

<PageTitle title="Documents" />

<div class="px-(--page-inset)">
	<PageHeader title="Documents">
		{#snippet actions()}
			<Button variant="secondary" disabled={creating} onclick={create}
				><Plus size={15} strokeWidth={1.75} aria-hidden="true" />New document</Button
			>
		{/snippet}
	</PageHeader>

	{#if documents.list.error && !documents.list.loaded}
		<EmptyState {...workspaceLoadError(documents.list.error, () => documents.list.refetch())} />
	{:else if documents.list.loaded}
		{#if documents.list.value.length === 0}
			<EmptyState message="No documents yet" actionLabel="New document" onaction={create} />
		{:else}
			<div class="mb-5">
				<SearchField
					bind:value={query}
					placeholder="Filter by title"
					label="Filter documents by title"
				/>
			</div>

			{#if groups.length === 0}
				{#if !remote || found !== null}
					<p class="text-fg-muted">No documents match “{query.trim()}”.</p>
				{/if}
			{:else}
				{#each groups as group (group.key)}
					<section class="[&+section]:mt-6" aria-labelledby="group-{group.key}">
						<h2 class="mb-1 text-xs font-medium text-fg-subtle" id="group-{group.key}">
							{group.label}
						</h2>
						<ul>
							{#each group.items as doc (doc.id)}
								<li class="border-border not-first:border-t">
									<a
										class="group flex items-baseline justify-between gap-4 py-2 text-fg hover:no-underline"
										href={routes.document(ws, doc.id)}
									>
										<span class="min-w-0 truncate transition-colors group-hover:text-accent"
											>{doc.title}</span
										>
										<span class="flex-none text-xs text-fg-subtle tabular-nums"
											>{relativeTime(doc.modified, now)}</span
										>
									</a>
								</li>
							{/each}
						</ul>
					</section>
				{/each}
			{/if}
		{/if}
	{/if}
</div>
