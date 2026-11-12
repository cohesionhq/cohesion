<!--
	A color from the agents' palette as a small swatch that opens a picker of every color and
	"No color". A table's options and a task folder take theirs here. Picking one closes it.
-->
<script lang="ts">
	import type { IntelligenceAgentColor } from '$lib/protocol/types';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { agentFill } from '$lib/ui/colors';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		color: IntelligenceAgentColor | null;
		/** What the color is of, for the control's accessible name: "Color of Read". */
		label: string;
		onchange: (color: IntelligenceAgentColor | null) => void;
		/** A smaller swatch and button, for a row's tools. */
		compact?: boolean;
	}

	let { color, label, onchange, compact = false }: Props = $props();

	let open = $state(false);

	const choices = $derived([
		{ value: null, label: 'No color' },
		...options(vocabulary.value.agent_colors)
	]);

	function pick(next: IntelligenceAgentColor | null) {
		open = false;
		if (next !== color) onchange(next);
	}
</script>

{#snippet swatch(value: IntelligenceAgentColor | null, small: boolean)}
	<span
		class={[
			'inline-block flex-none rounded-full',
			small ? 'size-2.5' : 'size-3',
			value ? agentFill[value] : 'border border-border-strong'
		]}
		aria-hidden="true"
	></span>
{/snippet}

<Popover bind:open inDialog>
	{#snippet trigger(props)}
		<button
			class={[
				'inline-flex flex-none cursor-pointer items-center justify-center rounded-sm hover:bg-bg-hover data-[state=open]:bg-bg-hover',
				compact ? 'size-6' : 'size-8'
			]}
			type="button"
			aria-label={label}
			{...props}
		>
			{@render swatch(color, compact)}
		</button>
	{/snippet}
	<div class="grid grid-cols-3 gap-1" role="group" aria-label={label}>
		{#each choices as choice (choice.value)}
			<button
				class="inline-flex size-8 cursor-pointer items-center justify-center rounded-sm hover:bg-bg-hover aria-pressed:bg-bg-hover"
				type="button"
				aria-label={choice.label}
				aria-pressed={choice.value === color}
				onclick={() => pick(choice.value)}
			>
				<span
					class={[
						'inline-flex rounded-full p-0.5 ring-fg-muted',
						choice.value === color && 'ring-2'
					]}
					aria-hidden="true"
				>
					{@render swatch(choice.value, false)}
				</span>
			</button>
		{/each}
	</div>
</Popover>
