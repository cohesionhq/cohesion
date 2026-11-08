<!--
	What agents remember: the global memories every agent sees, then each
	team's under its title, then each agent's own under its avatar and name.
	Forget takes one away after a confirmation.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { Memory } from '$lib/protocol/types';
	import { memories } from '$lib/stores/settings/memories.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { noLongerSeen, type Seers } from './forgetMemory';
	import Section from './Section.svelte';

	/** The memory being forgotten, and who sees it. */
	let forgetting = $state<{ memory: Memory; seers: Seers } | null>(null);

	$effect(() => {
		untrack(() => void memories.load());
		return () => memories.release();
	});

	const withMemories = $derived(memories.list.value.agents.filter((a) => a.memories.length > 0));
	const teamsWithMemories = $derived(
		memories.list.value.teams.filter((t) => t.memories.length > 0)
	);
</script>

{#snippet rows(items: Memory[], seers: Seers, empty: string)}
	<ul class={list.rows}>
		{#each items as memory (memory.id)}
			<li class={list.row}>
				<p class={[list.main, 'whitespace-pre-wrap']}>{memory.memory}</p>
				<div class={list.actions}>
					<Button variant="danger-quiet" onclick={() => (forgetting = { memory, seers })}>
						Forget…
					</Button>
				</div>
			</li>
		{:else}
			<li class={list.empty}>{empty}</li>
		{/each}
	</ul>
{/snippet}

<ResourceState resource={memories.list}>
	{#snippet loaded()}
		<Section title="Global" description="Every agent sees these.">
			{@render rows(memories.list.value.global, { kind: 'global' }, 'No global memories yet.')}
		</Section>
		{#if memories.list.value.teams.length > 0}
			<Section title="Teams" description="Each team has memories that its agents share.">
				{#each teamsWithMemories as { team, memories: shared } (team.id)}
					<h3 class="mt-4 mb-2 font-medium first:mt-0">{team.title}</h3>
					{@render rows(shared, { kind: 'team', title: team.title }, 'No memories yet.')}
				{:else}
					<p class={list.empty}>No team memories yet.</p>
				{/each}
			</Section>
		{/if}
		<Section title="Agents" description="Each agent has memories that only it sees.">
			{#each withMemories as { agent, memories: own } (agent.id)}
				<div class="mt-4 mb-2 flex items-center gap-2 first:mt-0">
					<AgentAvatar
						name={agent.agent_name}
						color={agent.agent_color}
						emoji={agent.agent_emoji}
						size="xs"
					/>
					<h3 class="font-medium">{agent.agent_name}</h3>
				</div>
				{@render rows(own, { kind: 'agent', name: agent.agent_name }, 'No memories yet.')}
			{:else}
				<p class={list.empty}>No agent memories yet.</p>
			{/each}
		</Section>
	{/snippet}
</ResourceState>

<ConfirmDialog
	subject={forgetting}
	onclose={() => (forgetting = null)}
	title="Forget this memory?"
	description={forgetting ? noLongerSeen(forgetting.seers) : ''}
	confirmLabel="Forget"
	onconfirm={({ memory }) => memories.forget(memory.id)}
/>
