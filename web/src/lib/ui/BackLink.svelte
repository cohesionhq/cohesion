<!--
	The way back, as the chevron alone leading a page's or a conversation's
	header before its title, or as "‹ {label}" where the header has no title. It
	steps back to the page the user came from, named by that page's title;
	reached any other way, it leads to the page given, the one this one sits
	under.
-->
<script lang="ts">
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import type { Back } from '$lib/shell/back';
	import { paneChrome } from './paneChrome';

	interface Props extends Back {
		/** Draws the chevron alone; the label is then its accessible name, "Back to {label}". */
		iconOnly?: boolean;
	}

	let { href, label, iconOnly = false }: Props = $props();

	const chrome = paneChrome();
	const previous = $derived(chrome?.back);
	const name = $derived(previous?.label ?? label);

	// A plain click steps back through history, so the page comes back as it was left.
	function onclick(event: MouseEvent) {
		if (
			!chrome ||
			!previous ||
			event.button !== 0 ||
			event.metaKey ||
			event.ctrlKey ||
			event.shiftKey
		)
			return;
		event.preventDefault();
		chrome.goBack();
	}
</script>

{#if iconOnly}
	<a
		class="-mr-1 inline-flex size-7 flex-none items-center justify-center rounded-sm text-fg-muted no-underline transition-colors hover:bg-bg-hover hover:text-fg"
		href={previous?.href ?? href}
		aria-label="Back to {name}"
		{onclick}
	>
		<ChevronLeft size={18} strokeWidth={1.75} aria-hidden="true" />
	</a>
{:else}
	<a
		class="-ml-1 flex h-7 w-fit items-center gap-0.5 rounded-sm pr-1.5 text-sm text-fg-muted no-underline hover:text-fg hover:no-underline"
		href={previous?.href ?? href}
		{onclick}><ChevronLeft size={14} strokeWidth={1.75} aria-hidden="true" />{name}</a
	>
{/if}
