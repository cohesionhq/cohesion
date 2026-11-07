<!--
	A conversation's pinned header: a frosted bar across the top of the page's
	sheet, edge to edge, that the feed scrolls under. One row: what leads it
	(a back link or an avatar), the title over a quiet line, then whatever
	trails it. Its insets are its own, the same on both sides, not the feed's.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { slide } from 'svelte/transition';
	import { motion } from './motion';
	import { paneChrome } from './paneChrome';
	import Shimmer from './Shimmer.svelte';

	interface Props {
		title: string;
		/** An agent at work, whose name in the title shows it. */
		working?: boolean;
		/** Quiet text under the title (a role, or where an actor works). */
		subtitle?: Snippet;
		/** Classes for the quiet line, for a page that draws it elsewhere when wide. */
		subtitleClass?: string;
		leading?: Snippet;
		trailing?: Snippet;
	}

	let { title, working = false, subtitle, subtitleClass, leading, trailing }: Props = $props();

	const controls = paneChrome()?.controls;
</script>

<header
	class="-mx-(--page-inset) flex min-h-14 items-center gap-2.5 border-b border-border bg-[color-mix(in_srgb,var(--bg)_74%,transparent)] px-3.5 py-2 backdrop-blur-xl backdrop-saturate-150"
>
	{#if leading}{@render leading()}{/if}
	<div class="flex min-w-0 flex-1 flex-col">
		<h1 class="flex text-md leading-tight font-semibold">
			<Shimmer class="min-w-0 truncate" active={working}>{title}</Shimmer>
		</h1>
		{#if subtitle}
			<span
				class={['truncate text-xs text-fg-muted', subtitleClass]}
				transition:slide={{ duration: motion.duration }}>{@render subtitle()}</span
			>
		{/if}
	</div>
	{#if trailing}{@render trailing()}{/if}
	{#if controls}{@render controls()}{/if}
</header>
