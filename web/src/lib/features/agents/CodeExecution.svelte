<!-- A script the agent ran: its title, opening onto the code and how it ended. -->
<script lang="ts">
	import type { AgentCodeExecution } from '$lib/protocol/types';
	import { prettyJson } from '$lib/stores/transcript';
	import { preClass } from './disclosure';
	import ToolRow from './ToolRow.svelte';

	interface Props {
		execution: AgentCodeExecution;
	}

	let { execution }: Props = $props();

	const outcome = $derived(execution.outcome);
</script>

<ToolRow title={execution.title || 'Ran code'}>
	{#snippet trailing()}
		{#if outcome.status === 'failed'}
			<span class="flex-none text-danger">Failed</span>
		{:else if outcome.status === 'cancelled'}
			<span class="flex-none text-fg-subtle">Cancelled</span>
		{/if}
	{/snippet}
	<pre class={[preClass, 'text-fg-muted']}>{execution.code}</pre>
	{#if outcome.status === 'failed'}
		<pre class={[preClass, 'text-danger']}>{outcome.error}</pre>
	{:else if outcome.status === 'succeeded'}
		<div class="text-xs text-fg-subtle">Result</div>
		<pre class={[preClass, 'text-fg-muted']}>{prettyJson(outcome.result)}</pre>
	{/if}
</ToolRow>
