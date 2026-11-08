<!--
	A second column, on the canvas to the right of the page's sheet, that
	speaks for the conversation open in the main half: what `children` draws.
	It shows only on a wide screen, sliding in and out as the side panel does.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { slide, type SlideParams } from 'svelte/transition';
	import { motion } from '$lib/ui/motion';

	let { open, children }: { open: boolean; children: Snippet } = $props();

	/** Opens or closes the column's width, the page's sheet giving way, as it fades. */
	function column(node: Element, params: SlideParams) {
		const width = slide(node, { ...params, axis: 'x' });
		return { ...width, css: (t: number, u: number) => `${width.css?.(t, u)};opacity:${t}` };
	}
</script>

{#if open}
	<!-- The wrapper's width opens and closes; the column inside keeps its own, so rows slide rather
	     than rewrap. It keeps to the sheet's insets, top, end and foot, the sheet's own margin the gap
	     between them, and starts a pixel into that gap so its scroller doesn't clip the card's hairline. -->
	<div class="flex flex-none max-lg:hidden" in:column={motion.slideIn} out:column={motion.slideOut}>
		<aside
			class="-ml-px flex w-60 flex-none [scrollbar-width:none] flex-col overflow-y-auto pt-[max(var(--sheet-inset),var(--titlebar-inset))] pr-(--sheet-inset) pb-(--sheet-inset) pl-px"
			aria-label="About this conversation"
		>
			{@render children()}
		</aside>
	</div>
{/if}
