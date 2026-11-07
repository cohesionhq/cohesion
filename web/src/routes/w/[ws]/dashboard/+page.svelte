<script lang="ts">
	import Dashboard from '$lib/features/dashboard/Dashboard.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';

	const page = pane();
	const ws = $derived(Number(page.params.ws));
</script>

<PageTitle title="Dashboard" />

{#key ws}
	<Dashboard {ws} />
{/key}
