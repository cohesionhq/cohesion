<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Back } from '$lib/shell/back';
	import BackLink from './BackLink.svelte';
	import { paneChrome } from './paneChrome';

	interface Props {
		/** The page's heading; `heading` replaces it with something richer (an editable title). */
		title?: string;
		heading?: Snippet;
		/**
		 * The way back for deeper pages, leading the bar: to the page the user came from,
		 * or else to the page this one sits under.
		 */
		back?: Back;
		/** Drawn before the title, centred on it (e.g. an agent's color dot). */
		leading?: Snippet;
		/** Quiet text under or beside the title (e.g. a save indicator). */
		meta?: Snippet;
		actions?: Snippet;
		/** Tabs under the title (a `Tabs`), sitting on the header's rule. */
		tabs?: Snippet;
		/** Fills a bar along the header's rule to this share, 0 to 1 (e.g. how far through an article). */
		progress?: number;
		/** Classes for the split view's controls, for a page that draws them elsewhere when narrow. */
		controlsClass?: string;
	}

	let { title, heading, back, leading, meta, actions, tabs, progress, controlsClass }: Props =
		$props();

	const controls = paneChrome()?.controls;
</script>

<!--
	Every page's header, one bar the same on every page and as tall as a
	conversation's (`ChatHeader`): pinned to the top of the page's scroller,
	the content scrolling under it, running to the sheet's edges over a
	hairline. One row: the way back as a chevron before the title (or, with no
	title, as "‹ {label}" alone), the title, its meta, then the page's tools at
	the trailing edge, which is the sheet's right edge on every page. Tabs sit
	under the row, on the hairline. `--page-header-height` in tokens.css is this
	bar's height; change both together. Its own view-transition name lifts it
	out of a page navigation's fade, so it swaps in place (see tokens.css); a
	page draws only one.
-->
<header
	class="sticky top-0 z-10 -mx-(--page-inset) mb-5 border-b border-border bg-bg px-(--page-inset) [view-transition-name:page-header]"
>
	<div class="flex min-h-14 items-center justify-between gap-4 py-2">
		<div class="flex min-w-0 flex-1 items-center gap-x-2.5">
			{#if back}<BackLink {...back} iconOnly={Boolean(title || heading)} />{/if}
			{#if leading}<span class="flex items-center">{@render leading()}</span>{/if}
			{#if heading}{@render heading()}{:else if title}<h1
					class="truncate text-md leading-tight font-semibold"
				>
					{title}
				</h1>{/if}
			{#if meta}<div class="flex-none text-xs text-fg-muted">{@render meta()}</div>{/if}
		</div>
		{#if actions || controls}
			<div class="flex flex-none items-center gap-2">
				{@render actions?.()}{#if controls}<div class={['flex', controlsClass]}>
						{@render controls()}
					</div>{/if}
			</div>
		{/if}
	</div>
	{#if tabs}<div class="-mb-px">{@render tabs()}</div>{/if}
	{#if progress !== undefined}
		<div
			class="absolute inset-x-0 -bottom-px h-0.5 origin-left bg-accent"
			style:transform="scaleX({progress})"
		></div>
	{/if}
</header>
