<!-- A route's page under its layouts, outermost first, as the split view's second half draws it. -->
<script lang="ts">
	import RouteView from './RouteView.svelte';
	import type { RouteComponent } from './splitRoutes';

	interface Props {
		layouts: RouteComponent[];
		page: RouteComponent;
		data: unknown;
	}

	let { layouts, page, data }: Props = $props();

	const Layout = $derived(layouts[0]);
	const Page = $derived(page);
</script>

{#if Layout}
	<Layout {data}>
		<RouteView layouts={layouts.slice(1)} {page} {data} />
	</Layout>
{:else}
	<Page {data} />
{/if}
