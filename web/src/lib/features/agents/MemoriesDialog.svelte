<!--
	What memories an agent sees: its own, its team's and the global ones,
	each under who sees it, with the day it was remembered. Forget takes one
	away after a confirmation.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { noLongerSeen, type Seers } from '$lib/features/settings/forgetMemory';
	import { shortDate } from '$lib/format/dates';
	import type { IntelligenceAgent, Memory } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { memories } from '$lib/stores/settings/memories.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { list } from '$lib/ui/list';

	const { agents } = pageStores();

	interface Props {
		open?: boolean;
		agent: IntelligenceAgent;
	}

	let { open = $bindable(false), agent }: Props = $props();

	const seen = memories.seen;

	$effect(() => {
		if (!open) return;
		const id = agent.id;
		untrack(() => void seen.load(id));
		return () => seen.release();
	});

	const team = $derived(agents.team(agent.team_id));
	const scoped = (kind: Memory['scope']['kind']) =>
		seen.key === agent.id ? seen.value.filter((m) => m.scope.kind === kind) : [];

	/** The memory being forgotten, and who sees it. */
	let forgetting = $state<{ memory: Memory; seers: Seers } | null>(null);
</script>

{#snippet group(title: string, description: string, items: Memory[], seers: Seers)}
	<section class="flex flex-col">
		<h3 class="font-medium">{title}</h3>
		<p class="text-xs text-fg-subtle">{description}</p>
		<ul class={list.rows}>
			{#each items as memory (memory.id)}
				<li class={list.row}>
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<p class="wrap-anywhere whitespace-pre-wrap">{memory.memory}</p>
						<time
							class="text-xs text-fg-subtle"
							datetime={new Date(memory.created_at * 1000).toISOString()}
							>{shortDate(memory.created_at)}</time
						>
					</div>
					<div class={list.actions}>
						<Button variant="danger-quiet" onclick={() => (forgetting = { memory, seers })}>
							Forget…
						</Button>
					</div>
				</li>
			{:else}
				<li class={list.empty}>No memories yet.</li>
			{/each}
		</ul>
	</section>
{/snippet}

<Dialog bind:open title="Memories" size="wide">
	<div class="flex max-h-[60vh] flex-col gap-5 overflow-y-auto">
		{#if seen.error && !seen.loaded}
			<p class="text-fg-subtle">{seen.error.message}</p>
		{:else if seen.key === agent.id}
			{@render group(agent.agent_name, `Only ${agent.agent_name} can see these.`, scoped('agent'), {
				kind: 'agent',
				name: agent.agent_name
			})}
			{#if team}
				{@render group(team.title, `Members of “${team.title}” see these.`, scoped('team'), {
					kind: 'team',
					title: team.title
				})}
			{/if}
			{@render group('Global', 'All agents see these.', scoped('global'), { kind: 'global' })}
		{/if}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Close</Button>
	{/snippet}
</Dialog>

<ConfirmDialog
	subject={forgetting}
	onclose={() => (forgetting = null)}
	title="Forget this memory?"
	description={forgetting ? noLongerSeen(forgetting.seers) : ''}
	confirmLabel="Forget"
	onconfirm={({ memory }) => memories.forget(memory.id)}
/>
