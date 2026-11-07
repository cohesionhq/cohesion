<!-- The split view's second half: the page open in it, or, while blank, a way to pick one. -->
<script lang="ts">
	import { keyLabel } from './keys';
	import PaneEmpty from './PaneEmpty.svelte';
	import PaneHost from './PaneHost.svelte';
	import RouteView from './RouteView.svelte';
	import type { SplitPane } from './pane.svelte';

	interface Props {
		split: SplitPane;
		/** Opens ⌘K, which opens what is picked in this half while it is the one in use. */
		onsearch: () => void;
	}

	let { split, onsearch }: Props = $props();
</script>

<PaneHost side="split" pane={split} stores={split.stores} bind:root={split.root}>
	{#if split.shown?.page}
		<RouteView layouts={split.shown.layouts} page={split.shown.page} data={split.shown.data} />
	{:else if split.shown}
		<PaneEmpty message="Page not found" />
	{:else}
		<PaneEmpty
			message={`Search for a page, or ${keyLabel('alt')}-click a link, to open it here.`}
			actionLabel="Search"
			onaction={onsearch}
		/>
	{/if}
</PaneHost>
