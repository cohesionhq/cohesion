<!--
	Rewind's timeline: rows grouped by day, each a node on one rail running down
	the left, so a history reads as time going back.
-->
<script lang="ts" generics="T extends { modified: number }">
	import type { Snippet } from 'svelte';
	import { fly } from 'svelte/transition';
	import { localMidnight, shortDate } from '$lib/format/dates';
	import { groupByDay } from '$lib/stores/rewind.svelte';
	import { motion } from '$lib/ui/motion';

	interface Props {
		rows: T[];
		key: (row: T) => string;
		/** How a row's node is drawn: `current` filled, `gone` in red, otherwise hollow. */
		node: (row: T) => 'current' | 'gone' | 'plain';
		onchoose: (row: T) => void;
		row: Snippet<[T]>;
	}

	let { rows, key, node, onchoose, row }: Props = $props();

	const days = $derived(groupByDay(rows, localMidnight(), (s) => shortDate(s)));
	const order = $derived(new Map(rows.map((r, i) => [key(r), i])));
</script>

{#each days as day (day.key)}
	<section class="not-first:mt-4">
		<h3 class="mb-1 text-xs font-medium text-fg-subtle">{day.label}</h3>
		<ol class="relative">
			<span class="absolute top-0 bottom-0 left-[7px] w-px bg-border" aria-hidden="true"></span>
			{#each day.rows as r (key(r))}
				<li
					in:fly|global={{
						y: 6,
						duration: motion.duration,
						delay: Math.min(order.get(key(r)) ?? 0, 10) * 30
					}}
				>
					<button
						class="group relative flex min-h-11 w-full cursor-pointer items-start gap-3 rounded-sm py-2 pr-2 text-left transition-colors hover:bg-bg-hover"
						type="button"
						onclick={() => onchoose(r)}
					>
						<span
							class="relative z-1 mt-1.5 flex w-[15px] flex-none justify-center"
							aria-hidden="true"
						>
							<span
								class={[
									'size-2.5 rounded-full ring-3 ring-bg transition-transform group-hover:scale-125',
									node(r) === 'current'
										? 'bg-fg'
										: node(r) === 'gone'
											? 'bg-danger'
											: 'border-[1.5px] border-border-strong bg-bg'
								]}
							></span>
						</span>
						<span class="min-w-0 flex-1">{@render row(r)}</span>
					</button>
				</li>
			{/each}
		</ol>
	</section>
{/each}
