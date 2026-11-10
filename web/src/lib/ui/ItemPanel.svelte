<!--
	An item opened over the page, the same for every kind: the side panel,
	titled with the item followed by its kind's icon on a tile and what it
	is, `Task #42`, as its chip says it; then its details as rows of an
	icon, a label and a value, and `children`, its text, under a rule when it
	has details. Expanded, an item with details puts them in a column of their own beside its text,
	each scrolling on its own. An item with a page of its own takes `href`, which puts Open beside the panel's other actions; `link` puts a button before Open that copies a Markdown link to the item (`copyItemLink`), and what links to the item (`shell/Backlinks.svelte`) under its text. The item's main action (`leading`), then the actions and Open, sit in a toolbar along the panel's foot; Close alone stays at the top.
-->
<script lang="ts">
	import Link from '@lucide/svelte/icons/link';
	import type { Component, Snippet } from 'svelte';
	import type { ItemLink } from '$lib/format/itemLinks';
	import Backlinks from '$lib/shell/Backlinks.svelte';
	import { copyItemLink } from './copyLink';
	import SidePanel from './SidePanel.svelte';

	interface Props {
		open?: boolean;
		title: string;
		icon: Component;
		/** What the item is, `Task #42` or `Document`. */
		kind: string;
		/** The item's own page, which Open goes to. */
		href?: string;
		/**
		 * What the item is linked as: puts a "Copy link" button in the toolbar,
		 * and what links to it (`Backlinks`) under its text.
		 */
		link?: ItemLink;
		/** The link's title when it isn't `title`, as a journal entry's day is. */
		linkTitle?: string;
		/** The toolbar's leading end, at the panel's foot: the item's main action. */
		leading?: Snippet;
		/** Beside Open, at the toolbar's trailing end. */
		actions?: Snippet;
		/** `ItemDetail`s, one per row. */
		details?: Snippet;
		/** The item's text, under its details. */
		children?: Snippet;
		onOpenAutoFocus?: (event: Event) => void;
	}

	let {
		open = $bindable(false),
		title,
		icon: Icon,
		kind,
		href,
		link,
		linkTitle,
		leading,
		actions: extra,
		details,
		children,
		onOpenAutoFocus
	}: Props = $props();

	/** An item with details has two columns once expanded. */
	const split = $derived(details !== undefined);
</script>

{#snippet toolbar()}
	<div class="flex items-center gap-1.5">{@render leading?.()}</div>
	<div class="flex items-center gap-1.5">
		{@render extra?.()}
		{#if link}
			<button
				class="inline-flex size-7.5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
				type="button"
				aria-label="Copy link"
				onclick={() => copyItemLink(link, linkTitle ?? title)}
			>
				<Link size={16} strokeWidth={1.75} aria-hidden="true" />
			</button>
		{/if}
		{#if href}
			<a
				class="inline-flex h-7.5 items-center rounded-sm px-3 font-medium text-fg no-underline transition-colors hover:bg-bg-hover hover:no-underline"
				{href}
				onclick={() => (open = false)}>Open</a
			>
		{/if}
	</div>
{/snippet}

<SidePanel
	bind:open
	{title}
	{onOpenAutoFocus}
	{split}
	footer={leading || extra || href || link ? toolbar : undefined}
>
	{#snippet aside()}
		<span class="flex items-center gap-2 text-sm whitespace-nowrap text-fg-muted">
			<span
				class="inline-flex size-7 flex-none items-center justify-center rounded-sm border border-border bg-bg"
			>
				<Icon size={15} strokeWidth={1.75} aria-hidden="true" />
			</span>
			{kind}
		</span>
	{/snippet}
	<div
		class={[
			'flex flex-1 flex-col gap-4',
			split
				? 'group-data-expanded/panel:min-h-0 group-data-expanded/panel:flex-row group-data-expanded/panel:gap-0'
				: 'group-data-expanded/panel:mx-auto group-data-expanded/panel:w-full group-data-expanded/panel:max-w-4xl'
		]}
	>
		{#if details}
			<div
				class="group-data-expanded/panel:w-80 group-data-expanded/panel:flex-none group-data-expanded/panel:overflow-y-auto group-data-expanded/panel:border-r group-data-expanded/panel:border-border group-data-expanded/panel:px-4 group-data-expanded/panel:py-4"
			>
				<dl
					class="grid grid-cols-[max-content_minmax(0,1fr)] items-start gap-x-6 gap-y-1 text-sm group-data-expanded/panel:gap-x-4"
				>
					{@render details()}
				</dl>
			</div>
		{/if}
		{#if children || link}
			<div
				class={[
					'flex flex-1 flex-col',
					details &&
						'border-t border-border pt-4 group-data-expanded/panel:min-w-0 group-data-expanded/panel:overflow-y-auto group-data-expanded/panel:border-t-0 group-data-expanded/panel:px-6 group-data-expanded/panel:py-4'
				]}
			>
				<div
					class="flex flex-1 flex-col group-data-expanded/panel:w-full group-data-expanded/panel:max-w-4xl"
				>
					{@render children?.()}
					{#if link}<Backlinks {link} />{/if}
				</div>
			</div>
		{/if}
	</div>
</SidePanel>
