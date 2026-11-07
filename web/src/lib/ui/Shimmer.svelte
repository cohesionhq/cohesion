<!--
	Text that is in progress: an agent's name while it works, an actor's running
	tool call. A soft band crosses it, a moving mask that dims the text where the
	band is, so it stays grey in the text's own color on any row and in either
	theme, keeps its width, and moves nothing around it. Grey, not the accent:
	work in progress needs no attention. Under reduced motion the text stays
	still and dimmed.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import { shimmerOnPageClock } from './motion';

	interface Props {
		active: boolean;
		/** Classes for the text's box, which truncates itself when given `truncate`. */
		class?: ClassValue;
		children: Snippet;
	}

	let { active, class: className, children }: Props = $props();
</script>

<span class={[className, active && 'shimmer']} {@attach active && shimmerOnPageClock}
	>{@render children()}</span
>

<style>
	.shimmer {
		opacity: 0.6;
	}

	/* At rest the band sits clear of the text. The page clock moves it across on
	   each beat (shimmerOnPageClock), every copy together. */
	@media (prefers-reduced-motion: no-preference) {
		.shimmer {
			opacity: 1;
			mask-image: linear-gradient(100deg, #000 40%, rgb(0 0 0 / 0.4) 50%, #000 60%);
			mask-size: 300% 100%;
			mask-repeat: no-repeat;
		}
	}
</style>
