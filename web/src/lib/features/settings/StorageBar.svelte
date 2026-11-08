<!--
	The database's size over one bar split by area in colour, with a legend,
	as the iPhone's storage screen shows its total. `StorageAreas` lists the
	numbers.
-->
<script lang="ts">
	import { fileSize } from '$lib/format/bytes';
	import type { DatabaseSize } from '$lib/protocol/types';
	import { list } from '$lib/ui/list';
	import { storageRows } from './storageAreas';

	interface Props {
		size: DatabaseSize;
	}

	let { size }: Props = $props();

	const rows = $derived(storageRows(size));
</script>

<li class={[list.row, 'flex-col items-stretch']}>
	<div class="flex items-baseline justify-between gap-3">
		<span>Size</span>
		<span class="tabular-nums">{fileSize(size.bytes)}</span>
	</div>
	<!-- The legend names the colours; the rows below give the numbers. -->
	<div class="mt-1 flex h-3 gap-px overflow-hidden rounded-sm bg-n4" aria-hidden="true">
		{#each rows as row (row.label)}
			{#if row.bytes > 0 && size.bytes > 0}
				<div
					class={['h-full flex-none', row.fill]}
					style:width="{(row.bytes / size.bytes) * 100}%"
				></div>
			{/if}
		{/each}
	</div>
	<ul class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-fg-muted" aria-hidden="true">
		{#each rows as row (row.label)}
			<li class="flex items-center gap-1.5">
				<span class={['size-2 rounded-full', row.fill]}></span>{row.label}
			</li>
		{/each}
	</ul>
</li>
