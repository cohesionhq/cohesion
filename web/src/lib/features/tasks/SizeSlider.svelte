<!--
	A task's size on a slider, smallest to largest, named beside it by the
	vocabulary, with a button that clears it. A task with no size shows the
	track with no thumb, and a click or drag on it sets one. The size is
	saved once the thumb is let go, not at each step of a drag.
-->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { Slider } from 'bits-ui';
	import type { TaskSize } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { sizes } from './size';

	interface Props {
		size: TaskSize | null;
		/** The task's title, for the control's accessible name. */
		title: string;
		/** Called with the size let go on, or `null` to clear it. */
		onchange: (size: TaskSize | null) => void;
	}

	let { size, title, onchange }: Props = $props();

	const steps = $derived(sizes().map((s) => s.value));
	/** The size the thumb is on, moving ahead of `size` during a drag. */
	let shown = $derived<TaskSize | null>(size);
	const label = $derived(shown === null ? 'None' : vocabulary.size(shown));
	const middle = $derived(steps[Math.floor(steps.length / 2)] ?? 1);

	function commit(value: number) {
		const next = value as TaskSize;
		shown = next;
		if (next !== size) onchange(next);
	}
</script>

<div class="flex min-h-7 items-center gap-3">
	<Slider.Root
		type="single"
		class="relative flex h-5 w-28 flex-none cursor-pointer touch-none items-center select-none"
		min={steps[0]}
		max={steps[steps.length - 1]}
		step={steps}
		value={shown ?? middle}
		onValueChange={(value) => (shown = value as TaskSize)}
		onValueCommit={commit}
	>
		{#snippet children({ tickItems })}
			<span class="relative h-1 w-full grow overflow-hidden rounded-full bg-n4">
				{#if shown !== null}<Slider.Range class="absolute h-full bg-accent" />{/if}
			</span>
			{#each tickItems as { index } (index)}
				<Slider.Tick {index} class="size-1 rounded-full bg-fg-subtle" />
			{/each}
			<Slider.Thumb
				index={0}
				class={[
					'block size-3.5 rounded-full bg-(--knob) shadow-knob focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
					shown === null && 'opacity-0 focus-visible:opacity-100'
				]}
				aria-label="Size of {title}"
				aria-valuetext={label}
			/>
		{/snippet}
	</Slider.Root>
	<span class={shown === null ? 'text-fg-muted' : 'text-fg'}>{label}</span>
	{#if size !== null}
		<button
			class="-ml-1 inline-flex size-6 cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:bg-bg-hover hover:text-fg"
			type="button"
			aria-label="Clear the size of {title}"
			onclick={() => {
				shown = null;
				onchange(null);
			}}
		>
			<X size={14} strokeWidth={1.75} />
		</button>
	{/if}
</div>
