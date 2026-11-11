<!--
	A few choices laid out as tiles, each an icon and a label, to pick one
	from in a form: a column's type, or where its values come from. The picked
	tile takes the grey of a current item. Arrow keys move the pick, as in any
	radio group.
-->
<script lang="ts" module>
	import type { Component } from 'svelte';

	export interface Tile {
		value: string;
		label: string;
		icon: Component;
	}
</script>

<script lang="ts">
	interface Props {
		/** What the tiles pick, for the group's accessible name. */
		label: string;
		tiles: Tile[];
		value: string;
		/** Tiles in a row. */
		columns: 2 | 3;
		/** The icon over the label, for a grid of several rows; beside it otherwise. */
		stacked?: boolean;
		onchange?: (value: string) => void;
		/** Added to the grid, e.g. to change its columns where it is narrower. */
		class?: string;
	}

	let {
		label,
		tiles,
		value = $bindable(),
		columns,
		stacked = false,
		onchange,
		class: extra
	}: Props = $props();

	let group = $state<HTMLDivElement>();

	function pick(next: string) {
		if (next === value) return;
		value = next;
		onchange?.(next);
	}

	function onkeydown(event: KeyboardEvent) {
		const step =
			event.key === 'ArrowRight' || event.key === 'ArrowDown'
				? 1
				: event.key === 'ArrowLeft' || event.key === 'ArrowUp'
					? -1
					: 0;
		if (step === 0) return;
		event.preventDefault();
		const at = tiles.findIndex((tile) => tile.value === value);
		const next = (at + step + tiles.length) % tiles.length;
		pick(tiles[next].value);
		group?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[next]?.focus();
	}
</script>

<div
	bind:this={group}
	class={['grid gap-2', columns === 3 ? 'grid-cols-3' : 'grid-cols-2', extra]}
	role="radiogroup"
	aria-label={label}
>
	{#each tiles as tile (tile.value)}
		{@const checked = tile.value === value}
		<button
			class={[
				'flex cursor-pointer items-center rounded-sm border transition-colors focus-visible:shadow-focus focus-visible:outline-none',
				stacked ? 'flex-col justify-center gap-1.5 px-2 py-3' : 'gap-2 px-3 py-2',
				checked
					? 'border-border-strong bg-current-item text-fg'
					: 'border-border text-fg-muted hover:bg-bg-hover hover:text-fg'
			]}
			type="button"
			role="radio"
			aria-checked={checked}
			tabindex={checked ? 0 : -1}
			onclick={() => pick(tile.value)}
			{onkeydown}
		>
			<tile.icon size={stacked ? 18 : 16} strokeWidth={1.75} aria-hidden="true" />
			<span class={['truncate', stacked && 'max-w-full text-xs']}>{tile.label}</span>
		</button>
	{/each}
</div>
