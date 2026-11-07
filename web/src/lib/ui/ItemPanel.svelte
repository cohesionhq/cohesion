<!--
	An item opened over the page, the same for every kind: the side panel,
	titled with the item followed by its kind's icon on a tile and what it
	is, `Task #42`, as its chip says it; then its details as rows of an
	icon, a label and a value, and `children`, its text, under a rule when it
	has details. An item with a page of its own takes `href`, which puts Open beside the panel's other actions. The item's main action (`leading`), then the actions and Open, sit in a toolbar along the panel's foot; Close alone stays at the top.
-->
<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import SidePanel from './SidePanel.svelte';

	interface Props {
		open?: boolean;
		title: string;
		icon: Component;
		/** What the item is, `Task #42` or `Document`. */
		kind: string;
		/** The item's own page, which Open goes to. */
		href?: string;
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
		leading,
		actions: extra,
		details,
		children,
		onOpenAutoFocus
	}: Props = $props();
</script>

{#snippet toolbar()}
	<div class="flex items-center gap-1.5">{@render leading?.()}</div>
	<div class="flex items-center gap-1.5">
		{@render extra?.()}
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
	footer={leading || extra || href ? toolbar : undefined}
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
	<div class="flex flex-1 flex-col gap-4">
		{#if details}
			<dl class="grid grid-cols-[max-content_minmax(0,1fr)] items-start gap-x-6 gap-y-1 text-sm">
				{@render details()}
			</dl>
		{/if}
		{#if children}
			<div class={['flex flex-1 flex-col', details && 'border-t border-border pt-4']}>
				{@render children()}
			</div>
		{/if}
	</div>
</SidePanel>
