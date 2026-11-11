<!-- An agent's color, chosen from the vocabulary's colors as a row of dots. -->
<script lang="ts">
	import type { IntelligenceAgentColor } from '$lib/protocol/types';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import AgentDot from './AgentDot.svelte';

	interface Props {
		value?: IntelligenceAgentColor;
		/** The fieldset's legend. */
		label?: string;
	}

	let { value = $bindable('blue'), label = 'Color' }: Props = $props();

	const id = $props.id();
</script>

<fieldset class="flex flex-col gap-1">
	<legend class="mb-1 font-medium">{label}</legend>
	<div class="flex flex-wrap gap-1">
		{#each options(vocabulary.value.agent_colors) as option (option.value)}
			<label
				class="relative inline-flex size-8 cursor-pointer items-center justify-center rounded-sm hover:bg-bg-hover has-checked:bg-bg-hover has-focus-visible:shadow-focus"
			>
				<input
					class="peer sr-only"
					type="radio"
					name="{id}-color"
					value={option.value}
					bind:group={value}
				/>
				<span
					class="inline-flex rounded-full p-0.5 ring-fg-muted peer-checked:ring-2"
					aria-hidden="true"
				>
					<AgentDot color={option.value} size="lg" />
				</span>
				<span class="sr-only">{option.label}</span>
			</label>
		{/each}
	</div>
</fieldset>
