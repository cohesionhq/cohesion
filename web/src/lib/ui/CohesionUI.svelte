<!--
	A CohesionUI interface, such as the one a connector draws a resource
	with: its blocks top to bottom, with overviews in a row lined up as one
	list of labels and values, like the details at the top of a task.

	Spacing is one scale: blocks sit 8px apart and nothing else adds space
	between them, except a heading, which takes 16px more above it. Every
	block starts at the same left edge, and every overview's value starts
	at the same indent, whichever run of overviews it is in.
-->
<script lang="ts">
	import type { Block, Component, OverviewBlock } from '$lib/protocol/types';
	import CohesionUIBlock from './CohesionUIBlock.svelte';

	let { component }: { component: Component } = $props();

	type Run = { overviews: OverviewBlock[] } | { block: Block };

	/** The blocks, with each row of overviews gathered into one run. */
	const runs = $derived(
		component.blocks.reduce<Run[]>((runs, block) => {
			const last = runs.at(-1);
			if (block.kind !== 'overview') runs.push({ block });
			else if (last && 'overviews' in last) last.overviews.push(block);
			else runs.push({ overviews: [block] });
			return runs;
		}, [])
	);
</script>

<div class="flex flex-col gap-2 leading-(--leading) wrap-anywhere">
	{#each runs as run, i (i)}
		{#if 'overviews' in run}
			<dl
				class="grid grid-cols-[8rem_minmax(0,1fr)] gap-x-4 gap-y-2 max-sm:grid-cols-[6rem_minmax(0,1fr)]"
			>
				{#each run.overviews as overview, j (j)}
					<dt class="text-fg-muted">{overview.title}</dt>
					<dd class="min-w-0">
						<CohesionUIBlock block={overview.child} />
					</dd>
				{/each}
			</dl>
		{:else}
			<CohesionUIBlock block={run.block} />
		{/if}
	{/each}
</div>
