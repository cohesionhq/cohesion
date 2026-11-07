<!--
	Rewind: what was deleted from this workspace that can still come back, most
	recently deleted first, each row what it is, its name, when it went and who
	deleted it. A row opens Rewind's panel for the item, and restoring a version
	there brings it back. Live items open their history from their own symbol.
-->
<script lang="ts">
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import FileText from '@lucide/svelte/icons/file-text';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import { untrack } from 'svelte';
	import RewindPanel from '$lib/features/rewind/RewindPanel.svelte';
	import { longDay, shortDate, shortTime } from '$lib/format/dates';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { authorLabel, type DeletedItem } from '$lib/stores/rewind.svelte';
	import { storage } from '$lib/stores/settings/storage.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { list } from '$lib/ui/list';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';

	const { rewind } = pageStores();

	const page = pane();
	const ws = $derived(Number(page.params.ws));

	let opened = $state<DeletedItem | null>(null);
	let panelOpen = $state(false);

	$effect(() => {
		const id = ws;
		untrack(() => {
			void rewind.deleted.load(id);
			void storage.settings.load();
		});
		panelOpen = false;
		return () => {
			rewind.deleted.release();
			storage.settings.release();
		};
	});

	const days = $derived(storage.settings.value?.rewind_retention_days);

	const name = (item: DeletedItem) => (item.journal_date ? longDay(item.journal_date) : item.title);
	const icon = (item: DeletedItem) =>
		item.kind === 'task' ? CheckSquare : item.journal_date ? NotebookPen : FileText;
	const kindLabel = (item: DeletedItem) =>
		item.kind === 'task' ? 'Task' : item.journal_date ? 'Journal entry' : 'Document';

	function show(item: DeletedItem) {
		opened = item;
		panelOpen = true;
	}
</script>

<PageTitle title="Rewind" />

<div class="px-(--page-inset)">
	<PageHeader title="Rewind" />
	<p class="mb-4 text-fg-muted">
		Deleted tasks, documents and journal entries come back from here{#if days}&nbsp;for {days}
			{days === 1 ? 'day' : 'days'}{/if}. To undo a change, use Rewind on the item itself.
	</p>
	<ResourceState resource={rewind.deleted}>
		{#snippet loaded(items)}
			{#if items.length === 0}
				<EmptyState message="No deleted items" />
			{:else}
				<ul class={list.rows}>
					{#each items as item (`${item.kind}-${item.id}`)}
						{@const Icon = icon(item)}
						<li class="border-b border-border">
							<button
								class="flex min-h-11 w-full cursor-pointer flex-wrap items-baseline gap-x-2 py-2 text-left transition-colors hover:bg-bg-hover"
								type="button"
								onclick={() => show(item)}
							>
								<span class="self-center text-fg-subtle">
									<Icon size={15} strokeWidth={1.75} aria-label={kindLabel(item)} />
								</span>
								<span class="min-w-0 wrap-anywhere">{name(item)}</span>
								<span class="ml-auto text-xs text-fg-subtle">
									Deleted {shortDate(item.modified)}, {shortTime(
										item.modified
									)}{#if authorLabel(item)}&nbsp;by
										{authorLabel(item)}{/if}
								</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		{/snippet}
	</ResourceState>
</div>

{#if opened}
	<RewindPanel bind:open={panelOpen} item={opened} title={name(opened)} />
{/if}
