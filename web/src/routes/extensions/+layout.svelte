<!-- Extensions: loads waiting updates for the Installed tab's count and an extension's own page. -->
<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { extensions } from '$lib/stores/extensions.svelte';

	let { children }: { children: Snippet } = $props();

	$effect(() => {
		untrack(() => void extensions.updates.load());
		return () => extensions.updates.release();
	});
</script>

{@render children()}
