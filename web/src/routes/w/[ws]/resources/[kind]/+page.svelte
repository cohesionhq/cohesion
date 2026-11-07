<script lang="ts">
	import ResourcesView from '$lib/features/connectors/ResourcesView.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();
</script>

<ResourcesView ws={Number(page.params.ws)} kindId={Number(page.params.kind)} />
