<script lang="ts">
	import { untrack } from 'svelte';
	import { dayToDate, localDay } from '$lib/format/dates';
	import type { Document } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { excerpt, groupByMonth } from '$lib/stores/documents.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { documents } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const today = localDay();

	$effect(() => {
		const id = ws;
		untrack(() => void documents.archive.load(id));
	});

	const groups = $derived(groupByMonth(documents.archive.value));

	function href(doc: Document): string {
		return doc.journal_date === today ? routes.today(ws) : routes.journalEntry(ws, doc.id);
	}

	function day(doc: Document): { weekday: string; date: string } {
		const d = dayToDate(doc.journal_date!);
		return {
			weekday: d.toLocaleDateString(undefined, { weekday: 'short' }),
			date: String(d.getDate())
		};
	}
</script>

<PageTitle title="Journal" />

<div class="px-(--page-inset)">
	<PageHeader title="Journal" />

	{#if documents.archive.error && !documents.archive.loaded}
		<EmptyState
			{...workspaceLoadError(documents.archive.error, () => documents.archive.refetch())}
		/>
	{:else if documents.archive.loaded}
		{#if groups.length === 0}
			<EmptyState
				message="No entries yet"
				actionLabel="Write today’s entry"
				onaction={() => page.goto(routes.today(ws))}
			/>
		{:else}
			{#each groups as group (group.key)}
				<section class="[section+&]:mt-6" aria-labelledby="month-{group.key}">
					<h2 class="mb-1 text-xs font-medium text-fg-subtle" id="month-{group.key}">
						{group.label}
					</h2>
					<ul>
						{#each group.items as doc (doc.id)}
							{@const d = day(doc)}
							{@const line = excerpt(doc.contents)}
							<li class="border-border not-first:border-t">
								<a
									class="group/link flex items-baseline gap-4 py-2 text-fg hover:no-underline"
									href={href(doc)}
								>
									<span
										class="inline-flex w-[4.5em] flex-none justify-between text-xs text-fg-subtle tabular-nums"
									>
										<span>{d.weekday}</span>
										<span>{d.date}</span>
									</span>
									<span
										class={[
											'min-w-0 flex-1 truncate transition-colors group-hover/link:text-accent',
											line === '' && 'text-fg-subtle'
										]}
									>
										{line === '' ? 'Empty' : line}
									</span>
									{#if doc.journal_date === today}<span class="flex-none text-xs text-fg-subtle"
											>Today</span
										>{/if}
								</a>
							</li>
						{/each}
					</ul>
				</section>
			{/each}
		{/if}
	{/if}
</div>
