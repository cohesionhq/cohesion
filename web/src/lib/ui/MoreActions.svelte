<!-- A page header's ⋯ menu: the actions that don't earn a button of their own. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Menu, { type MenuItem } from './Menu.svelte';

	interface Props {
		items: MenuItem[];
	}

	let { items }: Props = $props();
</script>

<Menu {items}>
	{#snippet trigger(props)}
		<button
			class="inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
			type="button"
			aria-label="More actions"
			{...props}
		>
			<Ellipsis size={16} strokeWidth={1.75} />
		</button>
	{/snippet}
</Menu>
