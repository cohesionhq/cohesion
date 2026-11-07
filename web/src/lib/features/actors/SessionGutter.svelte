<!--
	The column beside an actor session: its actor, then where that works and
	the model it runs on, then how full its context is and its actor's plan
	limits, all as bars. Where it stands and Resolve stay in the header.
-->
<script lang="ts">
	import { nowSeconds } from '$lib/format/dates';
	import { tokenCount } from '$lib/format/usage';
	import type { Actor, ActorSession, PlanUsage } from '$lib/protocol/types';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import GutterGroup from '$lib/ui/GutterGroup.svelte';
	import { resetsLabel, shortLimitLabel, usagePercent } from './plan';
	import UsageBar from './UsageBar.svelte';
	import { modelLabel, scopeName } from './words';

	interface Props {
		session: ActorSession;
		/** Its actor, unless no client offers it any longer. */
		actor: Actor | null;
		/** The plan limits its actor draws on. */
		plan: PlanUsage[];
	}

	let { session, actor, plan }: Props = $props();

	const model = $derived(modelLabel(session, actor));
	const hasUsage = $derived(session.context !== null || plan.length > 0);
</script>

<!-- Who runs it and where, in the row level with the header, as the header's own subtitle says it
     below 1100px; where it stands and Resolve stay in the header. -->
<GutterCard>
	{#if actor}
		<p class="truncate text-sm text-fg">{actor.title}</p>
	{/if}
	<p class="truncate text-xs text-fg-muted">
		{[actor ? scopeName(actor) : null, model].filter(Boolean).join(' · ')}
	</p>
</GutterCard>

<!-- One meter: its name and the share used over the bar, and under it what the share is of or
     when it resets, each at the column's text size. -->
{#snippet meter(label: string, share: number, note: string | null)}
	<section class="flex flex-col gap-1">
		<div class="flex items-baseline justify-between gap-2 text-xs">
			<h2 class="font-normal text-fg">{label}</h2>
			<span class="text-fg-muted tabular-nums">{usagePercent(Math.min(share, 1))}</span>
		</div>
		<UsageBar {share} />
		{#if note}<p class="truncate text-xs text-fg-subtle tabular-nums">{note}</p>{/if}
	</section>
{/snippet}

{#if hasUsage}
	<GutterGroup>
		<div class="flex flex-col gap-3">
			{#if session.context}
				{@render meter(
					'Context',
					session.context.used_tokens / session.context.total_tokens,
					`${tokenCount(session.context.used_tokens)} of ${tokenCount(session.context.total_tokens)} tokens`
				)}
			{/if}
			{#each plan as limit, index (index)}
				{@render meter(
					shortLimitLabel(limit.label),
					limit.used,
					limit.resets_at != null ? resetsLabel(limit.resets_at, nowSeconds()) : null
				)}
			{/each}
		</div>
	</GutterGroup>
{/if}
