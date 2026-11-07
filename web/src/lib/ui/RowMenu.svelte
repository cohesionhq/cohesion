<!--
	A list row's overflow menu: a quiet ⋯ at the row's end that holds what can
	be done to it (rename, delete, move), so a list of rows stays calm.
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
			class="inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg"
			type="button"
			aria-label={label}
			{...props}
		>
			<Ellipsis size={16} strokeWidth={1.75} />
		</button>
	{/snippet}
</Menu>
