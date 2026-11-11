<!--
	An option's color as a small swatch that opens a picker of the agent
	colors, the palette a task's size also uses, and "No color", which draws
	the option as a tag is. Picking one closes it.
-->
<script lang="ts">
	import type { IntelligenceAgentColor } from '$lib/protocol/types';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import AgentDot from '$lib/features/agents/AgentDot.svelte';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		color: IntelligenceAgentColor | null;
		/** What the color is of, for the control's accessible name: "Color of Read". */
		label: string;
		onchange: (color: IntelligenceAgentColor | null) => void;
	}

	let { color, label, onchange }: Props = $props();

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

{#snippet swatch(value: IntelligenceAgentColor | null)}
	{#if value}
		<AgentDot color={value} size="lg" />
	{:else}
		<span class="inline-block size-3 flex-none rounded-full border border-border-strong"></span>
	{/if}
{/snippet}

<Popover bind:open inDialog>
	{#snippet trigger(props)}
		<button
			class="inline-flex size-8 flex-none cursor-pointer items-center justify-center rounded-sm hover:bg-bg-hover data-[state=open]:bg-bg-hover"
			type="button"
			aria-label={label}
			{...props}
		>
			{@render swatch(color)}
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
					{@render swatch(choice.value)}
				</span>
			</button>
		{/each}
	</div>
</Popover>
