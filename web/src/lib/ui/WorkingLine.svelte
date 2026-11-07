<!--
	The line under a conversation while the agent or actor in it works, the one
	place that says what it is doing. It sits in the feed's column, rises in like
	a message and collapses on its way out, so a following feed eases down onto
	the reply instead of dropping by its height. It keeps to one line, truncated,
	and new words fade in over the old in that one cell, so a change mid-turn moves
	nothing. Each hugs its text, so the shimmer's band crosses it in full.
-->
<script lang="ts">
	import { fade, fly, slide } from 'svelte/transition';
	import { motion } from './motion';
	import Shimmer from './Shimmer.svelte';
	import { workingCaption } from './workingCaption';

	interface Props {
		/** What the daemon says the agent is doing. An actor session has a status instead. */
		activity?: string | null;
	}

	let { activity }: Props = $props();

	const caption = $derived(workingCaption(activity));
</script>

<p
	class="mx-auto mt-4 grid w-full max-w-190 grid-cols-1 justify-items-start text-xs text-fg-subtle"
	aria-live="polite"
	in:fly={{ y: 8, duration: motion.duration }}
	out:slide={{ duration: motion.duration }}
>
	{#key caption}
		<span
			class="col-start-1 row-start-1 max-w-full"
			transition:fade={{ duration: motion.duration }}
		>
			<Shimmer class="block truncate" active>{caption}</Shimmer>
		</span>
	{/key}
</p>
