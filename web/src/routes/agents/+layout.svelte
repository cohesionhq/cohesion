<script lang="ts">
	import type { Snippet } from 'svelte';
	import AgentView from '$lib/features/agents/AgentView.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	let { children }: { children: Snippet } = $props();

	// One view for `/agents` and `/agents/[agent]`, so the list and composer
	// survive switching agents.
	const agentId = $derived(page.params.agent ? Number(page.params.agent) : null);
</script>

<AgentView {agentId} />
{@render children()}
