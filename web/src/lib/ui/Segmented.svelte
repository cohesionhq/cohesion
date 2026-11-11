<!--
	A few views of one thing side by side, one showing at a time, drawn as a
	segmented control: a grey track (recessed in the dark) with the chosen segment raised on it.
	Each segment can carry a count. Arrow keys move between segments, as a
	radio group's do.
-->
<script lang="ts" module>
	export interface Segment {
		id: string;
		label: string;
		/** How many the segment holds, quiet after its label. */
		count?: number;
	}
</script>

<script lang="ts">
	interface Props {
		/** What the control switches, for its accessible label. */
		label: string;
		segments: Segment[];
		value: string;
		onchange: (id: string) => void;
	}

	let { label, segments, value, onchange }: Props = $props();

	let buttons = $state<HTMLButtonElement[]>([]);

	function onkeydown(event: KeyboardEvent, index: number) {
		const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
		if (!step) return;
		event.preventDefault();
		const next = (index + step + segments.length) % segments.length;
		onchange(segments[next].id);
		buttons[next]?.focus();
	}
</script>

<div
	class="inline-flex gap-0.5 rounded-sm bg-n2 p-0.5 dark:bg-n0"
	role="radiogroup"
	aria-label={label}
>
	{#each segments as segment, index (segment.id)}
		{@const checked = segment.id === value}
		<button
			bind:this={buttons[index]}
			type="button"
			role="radio"
			aria-checked={checked}
			tabindex={checked ? 0 : -1}
			class={[
				'inline-flex h-6.5 cursor-pointer items-center gap-1.5 rounded-sm px-3 font-medium transition-colors focus-visible:shadow-focus',
				checked ? 'bg-n0 text-fg shadow-knob dark:bg-n4' : 'text-fg-muted hover:text-fg'
			]}
			onclick={() => onchange(segment.id)}
			onkeydown={(event) => onkeydown(event, index)}
		>
			{segment.label}
			{#if segment.count !== undefined}
				<span class="text-xs font-normal text-fg-subtle tabular-nums">{segment.count}</span>
			{/if}
		</button>
	{/each}
</div>
