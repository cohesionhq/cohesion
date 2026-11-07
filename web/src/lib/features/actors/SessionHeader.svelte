<!--
	An actor session's pinned header, like an agent's: the way back, the
	session's title over what its actor is, the device it runs on and where it
	works, as the new session's composer says it, then the model it runs on,
	where it stands and the actions at the trailing end. From 1100px, in the
	half the column right of the sheet speaks for, the actor and the model move
	to that column (`SessionGutter`); where it stands and Resolve stay here.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { Actor, ActorSession } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import BackLink from '$lib/ui/BackLink.svelte';
	import ChatHeader from '$lib/ui/ChatHeader.svelte';
	import { paneChrome } from '$lib/ui/paneChrome';
	import StatusPill from '$lib/ui/StatusPill.svelte';
	import ActorPlace from './ActorPlace.svelte';
	import { modelLabel } from './words';

	interface Props {
		session: ActorSession;
		/** Its actor, unless no client offers it any longer. */
		actor: Actor | null;
		/** Drawn at the trailing edge (Resolve). */
		actions?: Snippet;
	}

	let { session, actor, actions }: Props = $props();

	const model = $derived(modelLabel(session, actor));
	const inGutter = paneChrome()?.hasGutter ? 'lg:hidden' : '';
</script>

{#snippet place()}{#if actor}<ActorPlace {actor} />{/if}{/snippet}

<ChatHeader title={session.title} subtitle={actor ? place : undefined} subtitleClass={inGutter}>
	{#snippet leading()}
		<BackLink href={routes.actors()} label="actors" iconOnly />
	{/snippet}
	{#snippet trailing()}
		{#if model}
			<span class={['flex-none text-xs whitespace-nowrap text-fg-muted max-sm:hidden', inGutter]}
				>{model}</span
			>
		{/if}
		<!-- Where it stands and Resolve stay beside the title at every width: what to do about it belongs with what it is. -->
		<span class="flex min-w-0">
			<StatusPill status={session.status} label={vocabulary.actorSessionStatus(session.status)} />
		</span>
		{#if actions}
			<div class="flex flex-none gap-2">{@render actions()}</div>
		{/if}
	{/snippet}
</ChatHeader>
