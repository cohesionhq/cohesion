<!--
	The column beside an actor session: where it stands, how full its context
	is as a ring, its actor's plan limits as bars, then its actor, where that
	works and the model it runs on, one to a line, and Resolve at the column's foot.
-->
<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import Folder from '@lucide/svelte/icons/folder';
	import Monitor from '@lucide/svelte/icons/monitor';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import { nowSeconds } from '$lib/format/dates';
	import { tokenCount } from '$lib/format/usage';
	import type { Actor, ActorSession, PlanUsage } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import Button from '$lib/ui/Button.svelte';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import GutterGroup from '$lib/ui/GutterGroup.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import ContextRing from './ContextRing.svelte';
	import { resetsLabel, shortLimitLabel, usagePercent } from './plan';
	import UsageBar from './UsageBar.svelte';
	import { modelLabel, scopeName } from './words';

	interface Props {
		session: ActorSession;
		/** Its actor, unless no client offers it any longer. */
		actor: Actor | null;
		/** The plan limits its actor draws on. */
		plan: PlanUsage[];
		onresolve: (session: ActorSession) => void;
	}

	let { session, actor, plan, onresolve }: Props = $props();

	const model = $derived(modelLabel(session, actor));
	const hasUsage = $derived(session.context !== null || plan.length > 0);
</script>

<GutterCard>
	<p
		class={[
			'flex items-center gap-1.5',
			session.status === 'needsAttention' ? 'text-warn' : 'text-fg-muted'
		]}
	>
		<StatusIcon status={session.status} size={14} />{vocabulary.actorSessionStatus(session.status)}
	</p>
</GutterCard>

{#if hasUsage}
	<GutterGroup>
		{#if session.context}
			<div class="flex items-center gap-2">
				<ContextRing
					share={Math.min(session.context.used_tokens / session.context.total_tokens, 1)}
				/>
				<div class="flex min-w-0 flex-col">
					<span class="text-xs text-fg-muted">Context</span>
					<span class="text-2xs text-fg-subtle tabular-nums">
						{tokenCount(session.context.used_tokens)} of {tokenCount(session.context.total_tokens)} tokens
					</span>
				</div>
			</div>
		{/if}
		{#if plan.length > 0}
			<div
				class={['flex flex-col gap-2.5', session.context && 'mt-1.5 border-t border-border pt-3']}
			>
				{#each plan as limit, index (index)}
					<section class="flex flex-col gap-1">
						<div class="flex items-baseline gap-2">
							<h2 class="flex-none text-xs font-normal text-fg-muted">
								{shortLimitLabel(limit.label)}
							</h2>
							{#if limit.resets_at != null}
								<span class="min-w-0 truncate text-2xs text-fg-subtle tabular-nums">
									{resetsLabel(limit.resets_at, nowSeconds())}
								</span>
							{/if}
							<span class="ml-auto flex-none text-2xs text-fg-muted tabular-nums">
								{usagePercent(limit.used)}
							</span>
						</div>
						<UsageBar share={limit.used} />
					</section>
				{/each}
			</div>
		{/if}
	</GutterGroup>
{/if}

{#if actor || model}
	<GutterGroup>
		<div class="flex flex-col gap-1 text-xs text-fg-muted">
			{#if actor}
				<p class="flex items-baseline gap-1.5">
					<Monitor
						class="size-3.5 flex-none translate-y-0.5 text-fg-muted"
						strokeWidth={1.75}
						aria-hidden="true"
					/>
					<span class="min-w-0 wrap-anywhere">{actor.title}</span>
				</p>
				<p class="flex items-baseline gap-1.5">
					<Folder
						class="size-3.5 flex-none translate-y-0.5 text-fg-muted"
						strokeWidth={1.75}
						aria-hidden="true"
					/>
					<span class="min-w-0 wrap-anywhere">{scopeName(actor)}</span>
				</p>
			{/if}
			{#if model}
				<p class="flex items-baseline gap-1.5">
					<Sparkles
						class="size-3.5 flex-none translate-y-0.5 text-fg-muted"
						strokeWidth={1.75}
						aria-hidden="true"
					/>
					<span class="min-w-0 wrap-anywhere">{model}</span>
				</p>
			{/if}
		</div>
	</GutterGroup>
{/if}

{#if session.status !== 'resolved'}
	<div class="mx-2 mt-auto flex-none pt-4">
		<Button class="w-full justify-center gap-1.5" onclick={() => onresolve(session)}>
			<Archive size={15} strokeWidth={1.75} aria-hidden="true" />Resolve
		</Button>
	</div>
{/if}
