<script lang="ts">
	import ExtensionView from '$lib/features/extensions/ExtensionView.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();
</script>

<ExtensionView id={Number(page.params.id)} />
