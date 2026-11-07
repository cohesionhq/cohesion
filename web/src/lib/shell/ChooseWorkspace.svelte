<!-- A global page's body when no workspace is current: `message` says what it needs one for. -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import EmptyState from '$lib/ui/EmptyState.svelte';

	let { message }: { message: string } = $props();
</script>

<EmptyState {message} actionLabel="Choose a workspace" onaction={() => void goto(resolve('/'))} />
