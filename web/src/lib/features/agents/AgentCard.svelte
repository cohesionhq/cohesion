<!--
	An agent at a glance, for the sidebar's hover card: its avatar, name
	and role, what it is doing right now, and whether it has posted unread
	messages.
-->
<script lang="ts">
	import type { IntelligenceAgent } from '$lib/protocol/types';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';

	interface Props {
		agent: IntelligenceAgent;
	}

	let { agent }: Props = $props();

	const running = $derived(agent.activity ?? '');
</script>

<div class="flex flex-col gap-2">
	<div class="flex items-center gap-2.5">
		<AgentAvatar
			name={agent.agent_name}
			color={agent.agent_color}
			emoji={agent.agent_emoji}
			size="sm"
		/>
		<span class="min-w-0 flex-1 truncate font-medium">{agent.agent_name}</span>
	</div>
	{#if agent.agent_role}
		<p class="text-xs wrap-anywhere text-fg-muted">{agent.agent_role}</p>
	{/if}
	{#if running}
		<p class="text-xs wrap-anywhere text-fg-muted">
			<span class="text-fg-subtle">Now:</span>
			{running}
		</p>
	{/if}
	{#if agent.agent_unread}
		<p class="flex items-center gap-1.5 text-xs text-fg">
			<span class="size-1.5 flex-none rounded-full bg-accent" aria-hidden="true"></span>
			Unread messages
		</p>
	{/if}
</div>
