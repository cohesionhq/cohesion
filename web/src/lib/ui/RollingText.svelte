<!--
	A short value whose characters roll upward when they change: the old one
	leaves through the top as the new one rises in. Only the characters that
	differ move, matched from the right so "18k" to "19k" turns one digit. The
	first value is simply set; only later changes roll.
-->
<script lang="ts">
	import { fly } from 'svelte/transition';
	import { motion } from './motion';

	interface Props {
		text: string;
		/** What a screen reader says instead, when the characters don't read as the value. */
		label?: string;
		/**
		 * The digits to hold room for, filled from the right: the value's width
		 * stays put as it grows to this length, and a new leading digit rolls in
		 * like the rest. For tabular numbers.
		 */
		reserve?: number;
	}

	let { text, label, reserve = 0 }: Props = $props();

	/** Each character with its distance from the end, which is what keeps a slot's identity. */
	const slots = $derived(
		// U+2007, a figure space, is as wide as a tabular digit.
		Array.from(text.padStart(reserve, '\u2007')).map((char, index, all) => ({
			char,
			place: all.length - index
		}))
	);
</script>

<span class="inline-flex overflow-hidden"
	><span class="sr-only">{label ?? text}</span>
	{#each slots as slot (slot.place)}
		<span class="inline-grid" aria-hidden="true">
			{#key slot.char}
				<!-- An em dash, standing in for a digit, takes a digit's width and hangs into the slot before it. -->
				<span
					class={['col-start-1 row-start-1', slot.char === '—' && 'ml-[calc(1ch-1em)]']}
					in:fly={{ y: '100%', duration: motion.duration }}
					out:fly={{ y: '-100%', duration: motion.duration }}>{slot.char}</span
				>
			{/key}
		</span>
	{/each}
</span>
