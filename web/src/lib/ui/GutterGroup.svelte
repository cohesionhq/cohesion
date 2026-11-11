<!--
	One group in the column right of the sheet, flat on the canvas, set apart
	from the group above by a hairline, and inset a little from the card's edges.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { slide } from 'svelte/transition';
	import { motion } from './motion';

	interface Props {
		children: Snippet;
		/** Slides open and shut as it comes and goes, rather than appearing at once. */
		slides?: boolean;
	}

	let { children, slides = false }: Props = $props();
</script>

<div
	class="mx-2 flex flex-none flex-col gap-1.5 pt-3 [&+&]:mt-3 [&+&]:border-t [&+&]:border-border-strong/60"
	transition:slide={{ duration: slides ? motion.duration : 0 }}
>
	{@render children()}
</div>
