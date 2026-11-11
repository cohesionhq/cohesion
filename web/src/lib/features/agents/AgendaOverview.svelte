<!--
	An agent's agenda in the column beside its conversation, kept to two lines however long it is: a
	dot a step, the finished ones stacked in green, the step the agent is on in indigo and the rest
	stacked in grey, with that step's text under them. Hovering, focusing or tapping it opens the
	whole agenda as a timeline on a card over the conversation, as Rewind draws history.
-->
<script lang="ts">
	import { flip } from 'svelte/animate';
	import { fly } from 'svelte/transition';
	import type { AgentAgendaItem } from '$lib/protocol/types';
	import HoverCard from '$lib/ui/HoverCard.svelte';
	import { motion } from '$lib/ui/motion';

	interface Props {
		/** The agent's agenda, in order, finished items included. */
		agenda: AgentAgendaItem[];
	}

	let { agenda }: Props = $props();

	let open = $state(false);
	const finished = $derived(agenda.filter((item) => item.finished_at !== null));
	const unfinished = $derived(agenda.filter((item) => item.finished_at === null));
	// The step the agent is on: the first not yet finished.
	const current = $derived(unfinished[0]);
	const summary = $derived(`${finished.length} of ${agenda.length} done`);
</script>

<h2 class="flex items-baseline justify-between gap-2 text-xs font-medium text-fg-subtle">
	Agenda
	<span class="font-normal tabular-nums">{summary}</span>
</h2>
<HoverCard bind:open side="left" align="start" openDelay={150} width="w-100" arrow>
	{#snippet trigger(props)}
		<!-- A touch screen has no hover, so a tap opens and closes the card too. -->
		<button
			{...props}
			type="button"
			class="flex w-full cursor-pointer flex-col gap-1 rounded-sm border-0 bg-transparent p-0 text-left text-fg-muted hover:text-fg"
			aria-expanded={open}
			onclick={() => (open = !open)}
		>
			<span class="flex h-4 items-center" aria-hidden="true">
				{#each finished as item (item.id)}
					<span class="size-2.5 rounded-full bg-ok ring-2 ring-canvas not-first:-ml-1"></span>
				{/each}
				{#each unfinished as item, index (item.id)}
					{#if (index === 0 && finished.length > 0) || index === 1}
						<span class="h-px w-2.5 bg-border-strong"></span>
					{/if}
					<span
						class={[
							'rounded-full ring-2 ring-canvas',
							index === 0 ? 'size-3 bg-accent' : 'size-2.5 bg-fg-subtle/50',
							index > 1 && '-ml-1'
						]}
					></span>
				{/each}
			</span>
			<span class="truncate text-xs">{current?.text ?? 'All done'}</span>
		</button>
	{/snippet}
	<h3 class="mb-1.5 flex items-baseline justify-between gap-2 text-xs font-medium text-fg-subtle">
		Agenda
		<span class="font-normal tabular-nums">{summary}</span>
	</h3>
	<!-- One rail down the left with a node per step. -->
	<ol class="relative flex flex-col text-sm">
		<span class="absolute top-2.5 bottom-2.5 left-[7px] w-px bg-border" aria-hidden="true"></span>
		{#each agenda as item (item.id)}
			{@const done = item.finished_at !== null}
			{@const now = item.id === current?.id}
			<li
				class={[
					'flex items-start gap-2.5 py-0.5',
					done ? 'text-fg-subtle' : now ? 'text-fg' : 'text-fg-muted'
				]}
				aria-current={now ? 'step' : undefined}
				in:fly={{ y: 6, duration: motion.duration }}
				animate:flip={{ duration: motion.duration }}
			>
				<span class="sr-only">{done ? 'Done' : now ? 'Next' : 'To do'}, </span>
				<span
					class="relative z-1 flex h-5.25 w-[15px] flex-none items-center justify-center"
					aria-hidden="true"
				>
					<span
						class={[
							'size-2 rounded-full ring-3 ring-(--bg-floating) transition-colors',
							done ? 'bg-ok' : now ? 'bg-accent' : 'bg-fg-subtle/50'
						]}
					></span>
				</span>
				<span class="min-w-0 flex-1 wrap-anywhere">{item.text}</span>
			</li>
		{/each}
	</ol>
</HoverCard>
