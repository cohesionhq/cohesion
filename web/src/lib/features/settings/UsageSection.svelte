<script lang="ts">
	import { untrack } from 'svelte';
	import { shortDate, shortTime } from '$lib/format/dates';
	import { usd } from '$lib/format/money';
	import {
		costParts,
		hasTokenCounts,
		inputTokens,
		tokenCount,
		usageTotals
	} from '$lib/format/usage';
	import { routes } from '$lib/shell/routes';
	import { USAGE_LIMIT, usage } from '$lib/stores/settings/usage.svelte';
	import { list } from '$lib/ui/list';
	import Popover from '$lib/ui/Popover.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Section from './Section.svelte';

	$effect(() => {
		untrack(() => void usage.load());
		return () => usage.events.release();
	});

	const totals = $derived(usageTotals(usage.events.value));

	// Every header and body cell; the first and last sit flush with the table's edges.
	const cell = 'border-b border-border px-2 py-1 align-baseline first:pl-0 last:pr-0';
	const numeric = 'text-right whitespace-nowrap';
</script>

<Section title="Usage" description="This lists every model call and what it cost.">
	<ResourceState resource={usage.events}>
		{#snippet loaded(events)}
			<div class="overflow-x-auto overscroll-y-auto border-t border-border">
				<table class="w-full min-w-160 border-collapse text-xs tabular-nums">
					<thead>
						<tr class="text-fg-muted">
							<th class={[cell, 'text-left font-medium']}>When</th>
							<th class={[cell, 'text-left font-medium']}>Model</th>
							<th class={[cell, numeric, 'font-medium']}>Input</th>
							<th class={[cell, numeric, 'font-medium']}>Cached</th>
							<th class={[cell, numeric, 'font-medium']}>Output</th>
							<th class={[cell, numeric, 'font-medium']}>Cost</th>
						</tr>
					</thead>
					<tbody>
						{#each events as event (event.id)}
							{@const parts = costParts(event)}
							<tr>
								<td class={[cell, 'text-left whitespace-nowrap']}>
									<span class="block">{shortDate(event.created_at)}</span>
									<span class="block text-2xs text-fg-subtle">{shortTime(event.created_at)}</span>
								</td>
								<td class={[cell, 'text-left']}>
									<span class="block font-mono text-2xs wrap-anywhere"
										>{event.model_identifier}</span
									>
									<span class="block text-2xs text-fg-subtle">
										{event.provider_identifier}{#if event.thread_id !== null}
											· <a href={routes.thread(event.thread_id)}>thread</a>{/if}
									</span>
								</td>
								<td class={[cell, numeric]}>
									{hasTokenCounts(event) ? tokenCount(inputTokens(event)) : '—'}
								</td>
								<td class={[cell, numeric]}>
									{hasTokenCounts(event) ? tokenCount(event.cache_read_tokens) : '—'}
								</td>
								<td class={[cell, numeric]}>
									{hasTokenCounts(event) ? tokenCount(event.output_tokens) : '—'}
								</td>
								<td class={[cell, numeric]}>
									{#if parts.length > 0}
										<Popover align="end">
											{#snippet trigger(props)}
												<button
													class="cursor-pointer tabular-nums underline decoration-border decoration-dotted underline-offset-2 hover:decoration-fg-subtle"
													type="button"
													{...props}
												>
													{usd(event.total_cost)}
												</button>
											{/snippet}
											<dl
												class="grid grid-cols-[auto_auto] gap-x-4 gap-y-0.5 px-2 py-1.5 text-xs tabular-nums"
											>
												{#each parts as part (part.label)}
													<dt class="text-fg-muted">{part.label}</dt>
													<dd class="text-right">{usd(part.cost)}</dd>
												{/each}
												<dt class="border-t border-border pt-0.5 font-medium">Total</dt>
												<dd class="border-t border-border pt-0.5 text-right font-medium">
													{usd(event.total_cost)}
												</dd>
											</dl>
										</Popover>
									{:else}
										{usd(event.total_cost)}
									{/if}
								</td>
							</tr>
						{:else}
							<tr>
								<td
									class="border-b border-border py-3 text-left align-baseline text-fg-muted"
									colspan="6"
								>
									No model calls yet.
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if totals.calls > 0}
				<p class={list.note}>
					{usd(totals.cost)} across {totals.calls}
					{totals.calls === 1 ? 'call' : 'calls'}{totals.calls === USAGE_LIMIT
						? ` (the ${USAGE_LIMIT} most recent)`
						: ''}.
				</p>
			{/if}
		{/snippet}
	</ResourceState>
</Section>
