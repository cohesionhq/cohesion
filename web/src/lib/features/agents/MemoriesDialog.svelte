<!--
	What memories an agent sees, one list at a time: its own (Personal), its
	team's and the global ones, switched by a segmented control. Each row
	shows two lines of the memory, opening to the rest on a click, and the day
	it was remembered. Forget takes one away after a confirmation.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { noLongerSeen, type Seers } from '$lib/features/settings/forgetMemory';
	import { shortDate } from '$lib/format/dates';
	import type { IntelligenceAgent, Memory } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { memories } from '$lib/stores/settings/memories.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { list } from '$lib/ui/list';
	import Segmented from '$lib/ui/Segmented.svelte';

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

	type Kind = Memory['scope']['kind'];

	const team = $derived(agents.team(agent.team_id));
	const scoped = (kind: Kind) =>
		seen.key === agent.id ? seen.value.filter((m) => m.scope.kind === kind) : [];

	interface View {
		id: Kind;
		label: string;
		description: string;
		seers: Seers;
	}

	/** Each list the control switches between: who sees it, and the words for it. */
	const views = $derived.by(() => {
		const all: View[] = [
			{
				id: 'agent',
				label: 'Personal',
				description: `Only ${agent.agent_name} can see these.`,
				seers: { kind: 'agent', name: agent.agent_name }
			}
		];
		if (team)
			all.push({
				id: 'team',
				label: 'Team',
				description: `Members of “${team.title}” see these.`,
				seers: { kind: 'team', title: team.title }
			});
		all.push({
			id: 'global',
			label: 'Global',
			description: 'All agents see these.',
			seers: { kind: 'global' }
		});
		return all;
	});

	let chosen = $state<Kind>('agent');
	const view = $derived(views.find((v) => v.id === chosen) ?? views[0]);
	const items = $derived(scoped(view.id));

	/** The memories opened past their first two lines. */
	const expanded = new SvelteSet<number>();

	function toggle(id: number) {
		if (!expanded.delete(id)) expanded.add(id);
	}

	/** The memory being forgotten, and who sees it. */
	let forgetting = $state<{ memory: Memory; seers: Seers } | null>(null);
</script>

<Dialog bind:open title="Memories" size="wide">
	{#if seen.error && !seen.loaded}
		<p class="text-fg-subtle">{seen.error.message}</p>
	{:else if seen.key === agent.id}
		<div class="flex flex-col items-start gap-2 border-b border-border pb-3">
			<Segmented
				label="Whose memories"
				segments={views.map((v) => ({ id: v.id, label: v.label, count: scoped(v.id).length }))}
				value={view.id}
				onchange={(id) => (chosen = id as Kind)}
			/>
			<p class="text-xs text-fg-subtle">{view.description}</p>
		</div>
		<ul class={[list.rows, 'max-h-[55vh] overflow-y-auto']}>
			{#each items as memory (memory.id)}
				{@const whole = expanded.has(memory.id)}
				<li class="group/row flex items-start gap-3 border-border py-3 not-first:border-t">
					<div class="flex min-w-0 flex-1 flex-col gap-1">
						<button
							type="button"
							class={[
								'cursor-pointer text-left leading-relaxed wrap-anywhere whitespace-pre-wrap',
								!whole && 'line-clamp-2'
							]}
							aria-expanded={whole}
							onclick={() => toggle(memory.id)}>{memory.memory}</button
						>
						<time
							class="text-2xs text-fg-subtle"
							datetime={new Date(memory.created_at * 1000).toISOString()}
							>{shortDate(memory.created_at)}</time
						>
					</div>
					<div class={[list.actions, list.reveal]}>
						<Button
							variant="danger-quiet"
							onclick={() => (forgetting = { memory, seers: view.seers })}
						>
							Forget…
						</Button>
					</div>
				</li>
			{:else}
				<li class={list.empty}>No memories yet.</li>
			{/each}
		</ul>
	{/if}
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
