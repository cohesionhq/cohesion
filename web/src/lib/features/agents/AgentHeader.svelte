<!--
	The conversation's pinned header: the agent's avatar, its name over its role,
	its context size in tokens ("—k" while it has none), and the actions at the
	trailing end. While the agent works the name stands alone and shimmers: the
	foot of the conversation says what it is doing (`WorkingLine`).
	From 1100px, in the half the column right of the sheet speaks for, only the avatar, the name and
	the actions stay: the column shows its context size (`AgentGutter`).
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { IntelligenceAgent } from '$lib/protocol/types';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import ChatHeader from '$lib/ui/ChatHeader.svelte';
	import ContextTokens from './ContextTokens.svelte';
	import { paneChrome } from '$lib/ui/paneChrome';

	interface Props {
		agent: IntelligenceAgent;
		/** Drawn at the trailing edge (the overflow menu). */
		actions?: Snippet;
	}

	let { agent, actions }: Props = $props();

	const inGutter = paneChrome()?.hasGutter ? 'lg:hidden' : '';
	const working = $derived(agent.activity !== null);
</script>

{#snippet role()}{agent.agent_role}{/snippet}

<!-- Drawn afresh for each agent, so moving to another doesn't slide its role in or out. -->
{#key agent.id}
	<ChatHeader
		title={agent.agent_name}
		{working}
		subtitle={agent.agent_role && !working ? role : undefined}
		subtitleClass={inGutter}
	>
		{#snippet leading()}
			<AgentAvatar
				name={agent.agent_name}
				color={agent.agent_color}
				emoji={agent.agent_emoji}
				size="md"
				tight
			/>
		{/snippet}
		{#snippet trailing()}
			<span class={['flex flex-none flex-col items-center leading-tight', inGutter]}>
				<span class="text-sm text-fg-muted tabular-nums"
					><ContextTokens tokens={agent.context_tokens} /></span
				>
				<span class="text-xs text-fg-subtle">tokens</span>
			</span>
			{#if actions}
				<div class="flex flex-none gap-2">{@render actions()}</div>
			{/if}
		{/snippet}
	</ChatHeader>
{/key}
