<!--
	An agent's panel (`ItemPanel`): its role, personality, verbosity, when
	it last wrote and when it was made, then its directive, with Open going
	to its conversation.
-->
<script lang="ts">
	import BriefcaseBusiness from '@lucide/svelte/icons/briefcase-business';
	import CalendarPlus from '@lucide/svelte/icons/calendar-plus';
	import Drama from '@lucide/svelte/icons/drama';
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import TextAlignStart from '@lucide/svelte/icons/text-align-start';
	import Bot from '@lucide/svelte/icons/bot';
	import { ago, longDate } from '$lib/format/dates';
	import type { IntelligenceAgent } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';

	let { open = $bindable(false), agent }: { open?: boolean; agent: IntelligenceAgent } = $props();

	const personality = $derived(
		vocabulary.value.agent_personalities.find((p) => p.kind === agent.agent_personality)?.label
	);
	const verbosity = $derived(
		vocabulary.value.agent_verbosities.find((v) => v.kind === agent.agent_verbosity)?.label
	);
</script>

<ItemPanel bind:open title={agent.agent_name} icon={Bot} kind="Agent" href={routes.agent(agent.id)}>
	{#snippet details()}
		<ItemDetail label="Role" icon={BriefcaseBusiness}>{agent.agent_role || 'None'}</ItemDetail>
		<ItemDetail label="Personality" icon={Drama}>{personality}</ItemDetail>
		<ItemDetail label="Verbosity" icon={TextAlignStart}>{verbosity}</ItemDetail>
		<ItemDetail label="Last message" icon={MessageSquare}>
			{agent.last_message_at === null ? 'None yet' : ago(agent.last_message_at)}
		</ItemDetail>
		<ItemDetail label="Created" icon={CalendarPlus}>{longDate(agent.created_at)}</ItemDetail>
	{/snippet}
	<h3 class="mb-2 text-sm font-medium text-fg-muted">Directive</h3>
	{#if agent.agent_directive}
		<p class="text-md whitespace-pre-wrap">{agent.agent_directive}</p>
	{:else}
		<p class="text-fg-subtle">None.</p>
	{/if}
</ItemPanel>
