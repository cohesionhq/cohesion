<!--
	An actor session's panel (`ItemPanel`): where it stands, its actor, model,
	when it started and last changed and how many messages it has, then the
	last thing its actor said, with Open going to its transcript.
-->
<script lang="ts">
	import CircleDot from '@lucide/svelte/icons/circle-dot';
	import CirclePlay from '@lucide/svelte/icons/circle-play';
	import Clock from '@lucide/svelte/icons/clock';
	import Cpu from '@lucide/svelte/icons/cpu';
	import Laptop from '@lucide/svelte/icons/laptop';
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import SquareTerminal from '@lucide/svelte/icons/square-terminal';
	import { ago, longDate, shortTime } from '$lib/format/dates';
	import type { Actor, ActorSession, ActorSessionMessage } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import StatusPill from '$lib/ui/StatusPill.svelte';
	import ActorPlace from './ActorPlace.svelte';
	import { modelLabel } from './words';

	interface Props {
		open?: boolean;
		session: ActorSession;
		actor: Actor | null;
		messages: ActorSessionMessage[];
	}

	let { open = $bindable(false), session, actor, messages }: Props = $props();

	const model = $derived(modelLabel(session, actor));
	const latest = $derived(
		messages.findLast(({ content }) => content.sender === 'actor' && content.kind === 'text')
	);
</script>

<ItemPanel
	bind:open
	title={session.title}
	icon={SquareTerminal}
	kind="Actor session"
	href={routes.actorSession(session.id)}
	link={{ kind: 'actor-session', ws: null, id: session.id }}
>
	{#snippet details()}
		<ItemDetail label="Status" icon={CircleDot}>
			<StatusPill status={session.status} label={vocabulary.actorSessionStatus(session.status)} />
		</ItemDetail>
		<ItemDetail label="Actor" icon={Laptop}>
			{#if actor}<ActorPlace {actor} />{:else}No longer offered{/if}
		</ItemDetail>
		<ItemDetail label="Model" icon={Cpu}>{model ?? 'Default'}</ItemDetail>
		<ItemDetail label="Started" icon={CirclePlay}
			>{longDate(session.started_at)}, {shortTime(session.started_at)}</ItemDetail
		>
		<ItemDetail label="Updated" icon={Clock}>{ago(session.status_changed_at)}</ItemDetail>
		<ItemDetail label="Messages" icon={MessageSquare}>{messages.length}</ItemDetail>
	{/snippet}
	<h3 class="mb-2 text-sm font-medium text-fg-muted">Latest from the actor</h3>
	{#if latest && latest.content.sender === 'actor' && latest.content.kind === 'text'}
		<div class="text-md"><Markdown source={latest.content.text} diagrams /></div>
	{:else}
		<p class="text-fg-subtle">Nothing yet.</p>
	{/if}
</ItemPanel>
