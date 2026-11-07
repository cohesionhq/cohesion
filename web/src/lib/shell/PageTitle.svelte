<!-- Names the page: the window's title in the main half, the back link's label in the second. -->
<script lang="ts">
	import { pane } from './pane.svelte';

	let { title }: { title: string } = $props();

	const here = pane();
	$effect(() => here.setTitle(title));
</script>
