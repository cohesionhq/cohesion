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
		 * A link above the title for deeper pages: back to the page the user came from,
		 * or else to this one, the page this one sits under.
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
	Pinned to the top of the page's scroller, the content scrolling under it.
	It draws no rule of its own, except under tabs, which sit on one. A back
	link takes its own row above the title rather than borrowing the header's
	top padding. `--page-header-height` in tokens.css adds up this
	header's spacing; change both together. Its own view-transition name lifts
	it out of a page navigation's fade, so it swaps in place (see tokens.css);
	a page draws only one.
-->
<header
	class={[
		'sticky top-0 z-10 bg-bg pt-3 [view-transition-name:page-header]',
		tabs ? 'mb-5 border-b border-border pb-0' : 'mb-1 pb-5'
	]}
>
	{#if back}<div class="mb-2"><BackLink {...back} /></div>{/if}
	<div class="box-content flex min-h-8 items-center justify-between gap-4">
		<div class={['flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1', heading && 'flex-1']}>
			{#if leading}<span class="-mr-1 flex items-center self-center">{@render leading()}</span>{/if}
			{#if heading}{@render heading()}{:else}<h1 class="text-lg">{title}</h1>{/if}
			{#if meta}<div class="text-xs text-fg-muted">{@render meta()}</div>{/if}
		</div>
		{#if actions || controls}
			<div class="flex flex-none items-center gap-2">
				{@render actions?.()}{#if controls}<div class={['flex', controlsClass]}>
						{@render controls()}
					</div>{/if}
			</div>
		{/if}
	</div>
	{#if tabs}<div class="mt-4">{@render tabs()}</div>{/if}
	{#if progress !== undefined}
		<div
			class="absolute inset-x-0 -bottom-px h-0.5 origin-left bg-accent"
			style:transform="scaleX({progress})"
		></div>
	{/if}
</header>
