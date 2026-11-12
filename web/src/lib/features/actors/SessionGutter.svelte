<!--
	The column beside an actor session: its actor, then where that works and
	the model it runs on, then, when it is a subagent or has any, the session
	it runs under and the subagents one level down from it, each opening its
	thread, then the agent monitoring it, with a button to stop it, then how full its context is and its actor's plan limits, all as
	bars. Only the open session's own relations show, not the whole tree.
	Where it stands and Resolve stay in the header.
-->
<script lang="ts">
	import { nowSeconds } from '$lib/format/dates';
	import { tokenCount } from '$lib/format/usage';
	import type { Actor, ActorSession, IntelligenceAgent, PlanUsage } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import CornerDownRight from '@lucide/svelte/icons/corner-down-right';
	import CornerUpLeft from '@lucide/svelte/icons/corner-up-left';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import AgentDot from '$lib/features/agents/AgentDot.svelte';
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
		/** For a subagent, the session one level up, once it has loaded. */
		above: ActorSession | null;
		/** The subagents launched from it, one level down, first launched first. */
		subagents: ActorSession[];
		/** The agent monitoring it, once the agents have loaded; `null` when none does. */
		monitor: IntelligenceAgent | null;
		/** Has `monitor` stop following the session, so it no longer wakes when the session stops. */
		onunfollow: (agent: IntelligenceAgent) => void;
	}

	let { session, actor, plan, above, subagents, monitor, onunfollow }: Props = $props();

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

<!-- One relation: the way up or down as a corner arrow, where it stands, and its title, opening its
     thread. -->
{#snippet relation(member: ActorSession, up: boolean)}
	{@const Arrow = up ? CornerUpLeft : CornerDownRight}
	<li>
		<a
			class="flex h-7 items-center gap-2 rounded-sm px-1 text-sm text-fg-muted no-underline transition-colors hover:bg-bg-hover hover:text-fg hover:no-underline"
			href={routes.actorSession(member.id)}
		>
			<Arrow class="flex-none text-fg-subtle" size={14} strokeWidth={1.75} aria-hidden="true" />
			<span class="sr-only">{up ? 'Runs under' : 'Subagent'}:</span>
			<StatusIcon status={member.status} unread={member.unread} size={12} />
			<span class="min-w-0 flex-1 truncate">{member.title}</span>
			<span class="sr-only">, {vocabulary.actorSessionStatus(member.status)}</span>
		</a>
	</li>
{/snippet}

<!-- Above the meters, so moving between relations keeps the list in place. A session with no
     relations shows none. -->
{#if above || subagents.length > 0}
	<GutterGroup>
		<ul class="-mx-1 flex flex-col" aria-label="Related sessions">
			{#if above}{@render relation(above, true)}{/if}
			{#each subagents as member (member.id)}{@render relation(member, false)}{/each}
		</ul>
	</GutterGroup>
{/if}

<!-- The agent that wakes when the session stops, opening its conversation, with Stop following
     on hover or focus, always shown on a touch screen. -->
{#if monitor}
	<GutterGroup>
		<h2 class="text-xs font-medium text-fg-subtle">Monitored by</h2>
		<div class="group/monitor relative">
			<a
				class="flex h-7 items-center gap-2 text-sm text-fg-muted no-underline hover:text-fg hover:no-underline touch:pr-8"
				href={routes.agent(monitor.id)}
			>
				<AgentDot color={monitor.agent_color} />
				<span class="min-w-0 flex-1 truncate">{monitor.agent_name}</span>
			</a>
			<button
				type="button"
				class="absolute top-1/2 right-0 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md border-0 bg-canvas text-fg-subtle opacity-0 group-focus-within/monitor:opacity-100 group-hover/monitor:opacity-100 hover:bg-bg-hover hover:text-fg touch:opacity-100"
				aria-label="Stop {monitor.agent_name} following “{session.title}”"
				onclick={() => onunfollow(monitor)}
			>
				<EyeOff size={14} strokeWidth={1.75} />
			</button>
		</div>
	</GutterGroup>
{/if}

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
