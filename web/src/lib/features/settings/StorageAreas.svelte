<!--
	Settings › Storage’s Usage: the bar of the database's size split by area,
	then each area's row with its colour, its size and its share of the
	database file, free space last.
-->
<script lang="ts">
	import { fileSize } from '$lib/format/bytes';
	import type { DatabaseSize } from '$lib/protocol/types';
	import { list } from '$lib/ui/list';
	import StorageBar from './StorageBar.svelte';
	import { storageRows, storageShare } from './storageAreas';

	interface Props {
		size: DatabaseSize;
	}

	let { size }: Props = $props();

	const rows = $derived(storageRows(size));
</script>

<ul class={list.rows}>
	<StorageBar {size} />
	{#each rows as row (row.label)}
		<li class={list.row}>
			<span class={['size-2.5 flex-none rounded-full', row.fill]} aria-hidden="true"></span>
			<div class={[list.main, 'flex-col']}>
				<span>{row.label}</span>
				{#if row.help}<span class="text-xs text-fg-subtle">{row.help}</span>{/if}
			</div>
			<div class="ml-auto flex flex-none items-baseline gap-3 tabular-nums">
				<span>{fileSize(row.bytes)}</span>
				<span class="w-9 text-right text-fg-muted">{storageShare(row.bytes, size.bytes)}</span>
			</div>
		</li>
	{/each}
</ul>
