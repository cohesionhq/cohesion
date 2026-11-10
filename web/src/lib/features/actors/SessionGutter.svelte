<!--
	The column beside an actor session: its actor, then where that works and
	the model it runs on, then how full its context is and its actor's plan
	limits, all as bars, then, when it has subagents or is one, its family:
	the session Cohesion started and every subagent beneath it, each under the
	one it runs under and set in by its depth, where it stands and what it is
	doing, the one open marked, each opening its thread. Where it stands and
	Resolve stay in the header.
-->
<script lang="ts">
	import { nowSeconds } from '$lib/format/dates';
	import { tokenCount } from '$lib/format/usage';
	import type { Actor, ActorSession, PlanUsage } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
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
		/**
		 * Its family: the session Cohesion started, then every subagent beneath
		 * it, each after the one it runs under; none until it loads.
		 */
		family: ActorSession[];
	}

	let { session, actor, plan, family }: Props = $props();

	/** How deep each member sits below the session Cohesion started, by id. */
	const depths = $derived(
		family.reduce<Record<number, number>>((depths, member) => {
			const above = member.subagent?.parent_session_id;
			depths[member.id] = above === undefined ? 0 : (depths[above] ?? 0) + 1;
			return depths;
		}, {})
	);

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

<!-- A session alone has no family to show. -->
{#if family.length > 1}
	<GutterGroup>
		<h2 class="text-xs font-medium text-fg-subtle">Session and subagents</h2>
		<ul class="-mx-1 flex flex-col">
			{#each family as member (member.id)}
				{@const open = member.id === session.id}
				<li>
					<a
						class={[
							'flex h-7 items-center gap-2 rounded-sm pr-1 text-sm no-underline transition-colors hover:bg-bg-hover hover:no-underline',
							open ? 'bg-bg-hover font-medium text-fg' : 'text-fg-muted hover:text-fg'
						]}
						style:padding-left="calc({depths[member.id] ?? 0} * var(--spacing) * 3.5 + var(--spacing))"
						href={routes.actorSession(member.id)}
						aria-current={open ? 'page' : undefined}
					>
						<StatusIcon status={member.status} unread={member.unread} size={12} />
						<span class="min-w-0 flex-1 truncate">{member.title}</span>
						<span class="sr-only">, {vocabulary.actorSessionStatus(member.status)}</span>
					</a>
				</li>
			{/each}
		</ul>
	</GutterGroup>
{/if}
