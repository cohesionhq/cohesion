<!--
	How much of its limits a session has used, as its actor last said: a
	ring that fills as the session's context window does (muted, so it never reads as
	a spinner; in the warning color past 80%, the danger color past 95%),
	which opens to the tokens as a bar and, under it, a bar for each of its
	actor's plan limits, with when each resets. Plan limits without a context
	window show behind a plain gauge instead.
-->
<script lang="ts">
	import Gauge from '@lucide/svelte/icons/gauge';
	import { nowSeconds } from '$lib/format/dates';
	import { tokenCount } from '$lib/format/usage';
	import type { ContextUsage, PlanUsage } from '$lib/protocol/types';
	import Popover from '$lib/ui/Popover.svelte';
	import { resetsLabel, usagePercent } from './plan';
	import ContextRing from './ContextRing.svelte';
	import UsageBar from './UsageBar.svelte';

	let { context, plan }: { context: ContextUsage | null; plan: PlanUsage[] } = $props();
	const filled = $derived(context ? Math.min(context.used_tokens / context.total_tokens, 1) : 0);

	const label = $derived(context ? `Context ${usagePercent(filled)} full` : 'Plan usage');
</script>

<Popover align="end" side="top" flyout>
	{#snippet trigger(props)}
		<button
			{...props}
			class="inline-flex size-8 flex-none cursor-pointer items-center justify-center rounded-full border border-border text-fg-muted transition-colors hover:border-border-strong hover:text-fg data-[state=open]:border-border-strong"
			type="button"
			aria-label={label}
		>
			{#if context}
				<ContextRing share={filled} />
			{:else}
				<Gauge size={16} strokeWidth={1.75} aria-hidden="true" />
			{/if}
		</button>
	{/snippet}
	<div class="flex w-64 flex-col gap-4 p-3">
		{#if context}
			<section class="flex flex-col gap-1.5">
				<div class="flex items-baseline justify-between gap-3">
					<h2 class="text-sm font-medium">Context</h2>
					<span class="text-xs text-fg-muted tabular-nums">{usagePercent(filled)}</span>
				</div>
				<UsageBar share={filled} />
				<p class="text-xs text-fg-subtle tabular-nums">
					{tokenCount(context.used_tokens)} of {tokenCount(context.total_tokens)} tokens
				</p>
			</section>
		{/if}
		{#each plan as limit, index (index)}
			<section class="flex flex-col gap-1.5">
				<div class="flex items-baseline justify-between gap-3">
					<h2 class="min-w-0 truncate text-sm font-medium">{limit.label}</h2>
					<span class="text-xs text-fg-muted tabular-nums">{usagePercent(limit.used)}</span>
				</div>
				<UsageBar share={limit.used} />
				{#if limit.resets_at != null}
					<p class="text-xs text-fg-subtle tabular-nums">
						{resetsLabel(limit.resets_at, nowSeconds())}
					</p>
				{/if}
			</section>
		{/each}
	</div>
</Popover>
