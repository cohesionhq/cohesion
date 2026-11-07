<script lang="ts">
	import SessionView from '$lib/features/actors/SessionView.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	const sessionId = $derived(Number(page.params.session));
</script>

<SessionView {sessionId} />
