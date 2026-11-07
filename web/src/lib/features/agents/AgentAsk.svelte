<!-- A question between agents, as one line saying who asked whom, opening onto the question and how it was answered. -->
<script lang="ts">
	import type { AgentAsk } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import Markdown from '$lib/ui/Markdown.svelte';
	import ToolRow from './ToolRow.svelte';

	interface Props {
		ask: AgentAsk;
		/** The message's excerpt: who asked whom, and the question. */
		title: string;
	}

	let { ask, title }: Props = $props();

	const reply = $derived(ask.reply);
</script>

<ToolRow {title}>
	{#snippet trailing()}
		{#if reply.status === 'waiting'}
			<span class="flex-none text-fg-subtle">Waiting</span>
		{:else if reply.status === 'unanswered'}
			<span class="flex-none text-danger">No answer</span>
		{/if}
	{/snippet}
	<a class="text-xs text-fg-subtle hover:text-fg" href={routes.agent(ask.agent.id)}>
		{ask.agent.name}
	</a>
	<Markdown source={ask.question} />
	{#if reply.status === 'answered'}
		<div class="text-xs text-fg-subtle">Answer</div>
		<Markdown source={reply.text} />
	{:else if reply.status === 'unanswered'}
		<p class="text-xs text-danger">{reply.reason}</p>
	{/if}
</ToolRow>
