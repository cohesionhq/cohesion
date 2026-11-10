<!--
	What links to an item (`backlinks`): under a quiet "Backlinks" label, one
	row per item whose text links to it, its kind's icon, its title and what it
	is. A click opens it over the page, as a link would (`linkedItem.follow`).
	Shows nothing while it loads, when nothing links to the item, or when the
	list can't be had.
-->
<script lang="ts">
	import FileText from '@lucide/svelte/icons/file-text';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import { longDay } from '$lib/format/dates';
	import { itemLinkHref, type ItemLink } from '$lib/format/itemLinks';
	import type { Embed } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { backlinks } from '$lib/stores/backlinks.svelte';
	import { linkedItem } from './linkedItem.svelte';
	import { itemLinkRoute, parseItemLink } from './routes';

	/** The item whose backlinks to show. */
	let { link }: { link: ItemLink } = $props();
	const id = $props.id();

	let shown = $state<ReturnType<typeof backlinks.show>>();

	$effect(() => {
		const current = backlinks.show(itemLinkHref(link));
		shown = current;
		return current.release;
	});

	const linking = $derived(shown?.backlinks.value ?? []);

	/** Only documents and journal entries link so far. */
	function icon(embed: Embed) {
		return embed.kind === 'journalEntry' ? NotebookPen : FileText;
	}

	/** A journal entry goes by its day, as its own panel titles it. */
	function title(embed: Embed): string {
		const { item } = embed;
		const day = item.kind === 'document' ? item.document.journal_date : null;
		return day ? longDay(day) : embed.title;
	}
</script>

{#if linking.length > 0}
	<section
		class="mt-6 flex flex-col gap-2 border-t border-border pt-4 first:mt-0"
		aria-labelledby="{id}-heading"
	>
		<h2 id="{id}-heading" class="text-xs font-medium text-fg-subtle">Backlinks</h2>
		<ul class="flex flex-col">
			{#each linking as embed (embed.link)}
				{@const target = parseItemLink(embed.link)}
				{@const Icon = icon(embed)}
				{#if target}
					<li>
						<a
							class="-mx-2 flex min-h-8 items-center gap-2 rounded-sm px-2 py-1 text-sm text-fg no-underline transition-colors hover:bg-bg-hover hover:no-underline"
							href={itemLinkRoute(target)}
							onclick={(event) => linkedItem.follow(event, target)}
						>
							<Icon
								class="flex-none text-fg-muted"
								size={15}
								strokeWidth={1.75}
								aria-hidden="true"
							/>
							<span class="min-w-0 truncate">{title(embed)}</span>
							<span class="flex-none text-xs text-fg-subtle"
								>{vocabulary.embedKind(embed.kind)}</span
							>
						</a>
					</li>
				{/if}
			{/each}
		</ul>
	</section>
{/if}
