<!--
	An agent's skills in the column beside its conversation, kept to one line: "Skills" and how many it
	has. Hovering, focusing or tapping it opens the list on a card over the conversation, as the
	agenda's does; clicking a skill there sends a message running that skill and nothing else.
-->
<script lang="ts">
	import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
	import type { AgentSkill } from '$lib/protocol/types';
	import HoverCard from '$lib/ui/HoverCard.svelte';

	interface Props {
		/** The skills the agent has, by name. */
		skills: AgentSkill[];
		/** Sends a message running `skill`, with no note. */
		onsend: (skill: AgentSkill) => void;
	}

	let { skills, onsend }: Props = $props();

	let open = $state(false);
	const summary = $derived(`${skills.length} available`);
</script>

<HoverCard bind:open side="left" align="start" openDelay={150} width="w-100" arrow>
	{#snippet trigger(props)}
		<!-- A touch screen has no hover, so a tap opens and closes the card too. -->
		<button
			{...props}
			type="button"
			class="flex w-full cursor-pointer items-baseline justify-between gap-2 rounded-sm border-0 bg-transparent p-0 text-left text-xs font-medium text-fg-subtle hover:text-fg"
			aria-expanded={open}
			onclick={() => (open = !open)}
		>
			Skills
			<span class="font-normal tabular-nums">{summary}</span>
		</button>
	{/snippet}
	<h3 class="mb-1.5 flex items-baseline justify-between gap-2 text-xs font-medium text-fg-subtle">
		Skills
		<span class="font-normal tabular-nums">{summary}</span>
	</h3>
	<ul class="flex flex-col gap-px">
		{#each skills as skill (skill.id)}
			<li>
				<button
					type="button"
					class="flex w-full cursor-pointer items-start gap-2 rounded-sm border-0 bg-transparent px-1 py-1 text-left text-sm text-fg-muted hover:bg-bg-hover hover:text-fg"
					aria-label="Send “{skill.name}”"
					onclick={() => {
						open = false;
						onsend(skill);
					}}
				>
					<!-- Centred on the name's line. -->
					<span class="flex h-5.25 flex-none items-center" aria-hidden="true">
						<WandSparkles class="size-4 text-fg-subtle" strokeWidth={1.75} />
					</span>
					<span class="flex min-w-0 flex-col">
						<span class="truncate text-fg">{skill.name}</span>
						{#if skill.description}
							<span class="truncate text-xs text-fg-subtle">{skill.description}</span>
						{/if}
					</span>
				</button>
			</li>
		{/each}
	</ul>
	<p class="mt-2 text-xs text-fg-subtle">Selecting a skill sends a message with just that skill.</p>
</HoverCard>
