<!--
	A remember or forget line in an agent's conversation, which opens a small
	panel beside it: the memory, who sees it (or that it's no longer kept),
	and a link to Settings › Memories, where memories are reviewed and
	forgotten.
-->
<script lang="ts">
	import Brain from '@lucide/svelte/icons/brain';
	import type { AgentMemoryChange, IntelligenceAgent, Memory } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { memories } from '$lib/stores/settings/memories.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Popover from '$lib/ui/Popover.svelte';

	const { agents } = pageStores();

	interface Props {
		agent: IntelligenceAgent;
		/** The memory's text, as the conversation recorded it. */
		text: string;
		change: AgentMemoryChange;
		/** The line's words. */
		line: string;
	}

	let { agent, text, change, line }: Props = $props();

	let open = $state(false);
	/** The memory still kept with this text; undefined until read, null when gone. */
	let kept = $state<Memory | null | undefined>(undefined);
	let failed = $state(false);

	// A remembered memory may have been forgotten since; the message has no
	// id, so it's matched to what the agent sees now by its text.
	$effect(() => {
		if (!open || change === 'forgotten') return;
		const id = agent.id;
		kept = undefined;
		failed = false;
		let stale = false;
		memories
			.seenBy(id)
			.then((found) => !stale && (kept = found.find((m) => m.memory === text) ?? null))
			.catch(() => !stale && (failed = true));
		return () => (stale = true);
	});

	const seers = (memory: Memory) => {
		switch (memory.scope.kind) {
			case 'global':
				return 'Every agent sees this.';
			case 'team': {
				const team = agents.team(memory.scope.team_id);
				return team ? `Agents on ${team.title} see this.` : 'Agents on its team see this.';
			}
			case 'agent':
				return `Only ${agent.agent_name} sees this.`;
		}
	};
</script>

<Popover bind:open align="start">
	{#snippet trigger(props)}
		<button
			class="flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 text-left text-xs leading-6 text-fg-subtle hover:text-fg data-[state=open]:text-fg"
			type="button"
			{...props}
		>
			<Brain class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
			<span class="min-w-0 truncate">{line}</span>
		</button>
	{/snippet}
	<div class="flex w-80 max-w-full flex-col gap-2 px-2 py-1.5">
		<p class="text-xs text-fg-subtle">
			{agent.agent_name}
			{change === 'forgotten' ? 'forgot' : 'remembered'}
		</p>
		<p class={['wrap-anywhere whitespace-pre-wrap', change === 'forgotten' && 'text-fg-muted']}>
			{text}
		</p>
		{#if change === 'remembered'}
			{#if failed}
				<p class="text-xs text-danger">Couldn’t load {agent.agent_name}’s memories.</p>
			{:else if kept !== undefined}
				<p class="text-xs text-fg-subtle">{kept ? seers(kept) : 'No longer remembered.'}</p>
			{/if}
		{/if}
		<a class="self-start text-xs" href={routes.settings('memories')} onclick={() => (open = false)}
			>Manage memories</a
		>
	</div>
</Popover>
