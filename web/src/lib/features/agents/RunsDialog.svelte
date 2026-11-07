<!-- An agent's recent runs, its jobs and forks: what woke each, how it went, what it cost. -->
<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import { untrack } from 'svelte';
	import { relativeTime } from '$lib/format/dates';
	import { usd } from '$lib/format/money';
	import { inputTokens, tokenCount } from '$lib/format/usage';
	import { routes } from '$lib/shell/routes';
	import type { IntelligenceAgentRun } from '$lib/protocol/types';
	import { hookLabel } from '$lib/stores/agents.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents } = pageStores();

	interface Props {
		open?: boolean;
		agentId: number;
	}

	let { open = $bindable(false), agentId }: Props = $props();

	const runs = agents.runs;
	let now = $state(Math.floor(Date.now() / 1000));

	$effect(() => {
		if (!open) return;
		const id = agentId;
		now = Math.floor(Date.now() / 1000);
		untrack(() => void runs.load(id));
		return () => runs.release();
	});

	/**
	 * What woke `run`: its cue's name, the agent whose question it answered,
	 * or, for a cue named after its hook, the hook.
	 */
	function woke(run: IntelligenceAgentRun): string {
		if (run.sender_agent_id !== null) {
			const sender = agents.agent(run.sender_agent_id);
			return sender ? `When ${sender.agent_name} asked it` : 'When another agent asked it';
		}
		return run.cue_name && run.cue_name !== run.hook ? run.cue_name : `When ${hookLabel(run.hook)}`;
	}

	function duration(started: number, ended: number): string {
		const seconds = ended - started;
		return seconds < 60 ? `${seconds}s` : `${Math.round(seconds / 60)}m`;
	}
</script>

<Dialog bind:open title="Runs" size="wide">
	<div class="max-h-[60vh] overflow-y-auto">
		{#if runs.error && !runs.loaded}
			<p class="text-fg-subtle">{runs.error.message}</p>
		{:else if runs.loaded && runs.key === agentId && runs.value.length === 0}
			<p class="text-fg-subtle">
				No runs yet. One starts each time one of its cues wakes it, another agent asks it something,
				or an actor session it follows moves on.
			</p>
		{:else if runs.key === agentId}
			<ul class="flex flex-col">
				{#each runs.value as run (run.id)}
					<li class="flex items-start gap-2.5 border-b border-border py-2 last:border-b-0">
						<span class="flex h-6 flex-none items-center">
							{#if run.state.status === 'running'}
								<LoaderCircle
									class="text-accent motion-safe:animate-spin"
									size={16}
									strokeWidth={1.75}
									aria-label="Running"
								/>
							{:else if run.state.status === 'completed'}
								<CircleCheck
									class="text-fg-subtle"
									size={16}
									strokeWidth={1.75}
									aria-label="Completed"
								/>
							{:else}
								<CircleX class="text-danger" size={16} strokeWidth={1.75} aria-label="Failed" />
							{/if}
						</span>
						<div class="flex min-w-0 flex-1 flex-col gap-0.5">
							<div class="flex items-baseline gap-3 leading-6">
								<span class="min-w-0 flex-1 font-medium wrap-anywhere">{woke(run)}</span>
								<time
									class="flex-none text-xs text-fg-subtle tabular-nums"
									datetime={new Date(run.started_at * 1000).toISOString()}
									>{relativeTime(run.started_at, now)}</time
								>
							</div>
							{#if run.cue_name && run.cue_name !== run.hook}
								<span class="text-xs text-fg-muted">When {hookLabel(run.hook)}</span>
							{/if}
							<div
								class="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-fg-subtle tabular-nums"
							>
								{#if run.state.status !== 'running'}<span
										>{duration(run.started_at, run.state.ended_at)}</span
									>{/if}
								<span class="inline-flex items-center gap-0.5">
									<ArrowUp size={12} strokeWidth={1.75} aria-label="Input tokens" />
									{tokenCount(inputTokens(run))}
								</span>
								<span class="inline-flex items-center gap-0.5">
									<ArrowDown size={12} strokeWidth={1.75} aria-label="Output tokens" />
									{tokenCount(run.output_tokens)}
								</span>
								{#if run.cost !== null}<span>{usd(run.cost)}</span>{/if}
								{#if run.thread_id !== null}
									<a class="ml-auto" href={routes.thread(run.thread_id)}>Transcript</a>
								{/if}
							</div>
							{#if run.state.status === 'failed' && run.state.error}
								<p class="text-xs wrap-anywhere text-danger">{run.state.error}</p>
							{/if}
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Close</Button>
	{/snippet}
</Dialog>
