<!--
	A list row's overflow menu: a quiet ⋯ at the row's end that holds what can
	be done to it (rename, delete, move), so a list of rows stays calm. It shows
	while its row (the `li` or `dd` around it) is hovered or holds focus, while
	open, and always on a touch screen.
-->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Menu, { type MenuItem } from './Menu.svelte';

	interface Props {
		/** Names the trigger for assistive tech: "More for Personal". */
		label: string;
		items: MenuItem[];
		/** Drawn inside a dialog or panel, so the menu must sit above it. */
		inDialog?: boolean;
	}

	let { label, items, inDialog = false }: Props = $props();
</script>

<Menu {items} {inDialog}>
	{#snippet trigger(props)}
		<button
			class="inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted opacity-0 transition-[color,background-color,opacity] hover:bg-bg-hover hover:text-fg focus-visible:opacity-100 data-[state=open]:bg-bg-hover data-[state=open]:text-fg data-[state=open]:opacity-100 touch:opacity-100 [dd:focus-within_&]:opacity-100 [dd:hover_&]:opacity-100 [li:focus-within_&]:opacity-100 [li:hover_&]:opacity-100 [tr:focus-within_&]:opacity-100 [tr:hover_&]:opacity-100"
			type="button"
			aria-label={label}
			{...props}
		>
			<Ellipsis size={16} strokeWidth={1.75} />
		</button>
	{/snippet}
</Menu>
