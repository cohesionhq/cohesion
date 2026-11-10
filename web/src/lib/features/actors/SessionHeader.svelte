<!--
	An actor session's pinned header, like an agent's: where it stands as its
	status disc, leading as an agent's avatar does, the session's title over
	where it stands in words and what its actor is, the device it runs on and
	where it works, as the new session's composer says it, then the model it
	runs on and the actions at the trailing end. From 1100px, in the half the
	column right of the sheet speaks for, the actor and the model move to that
	column (`SessionGutter`); where it stands and Resolve stay here.

	A subagent's header leads with the way back up to the session it runs
	under, before its disc, and says what kind of subagent it is, and whose.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { Actor, ActorSession } from '$lib/protocol/types';
	import { backOut } from 'svelte/easing';
	import { scale } from 'svelte/transition';
	import { routes } from '$lib/shell/routes';
	import BackLink from '$lib/ui/BackLink.svelte';
	import ChatHeader from '$lib/ui/ChatHeader.svelte';
	import { motion } from '$lib/ui/motion';
	import { paneChrome } from '$lib/ui/paneChrome';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import ActorPlace from './ActorPlace.svelte';
	import { modelLabel } from './words';

	interface Props {
		session: ActorSession;
		/** Its actor, unless no client offers it any longer. */
		actor: Actor | null;
		/** Drawn at the trailing edge (Resolve). */
		actions?: Snippet;
		/** For a subagent, the session one level up, once it has loaded. */
		above?: ActorSession | null;
	}

	let { session, actor, actions, above = null }: Props = $props();

	const subagent = $derived(session.subagent);
	/** What kind of subagent it is, in words: "Explore subagent". */
	const subagentKind = $derived(
		subagent?.agent_type ? `${subagent.agent_type} subagent` : 'Subagent'
	);

	const model = $derived(modelLabel(session, actor));
	const inGutter = paneChrome()?.hasGutter ? 'lg:hidden' : '';
</script>

<!-- Where it stands in words, then who runs it and where; from 1100px the column beside says the second. -->
{#snippet subtitle()}
	<span class={session.status === 'needsAttention' ? 'text-warn' : undefined}
		>{vocabulary.actorSessionStatus(session.status)}</span
	>{#if subagent}<span
			>&nbsp;<span aria-hidden="true">·</span>
			{subagentKind}{#if above}&#32;of <a class="text-fg-muted" href={routes.actorSession(above.id)}
					>{above.title}</a
				>{/if}</span
		>
	{:else if actor}<span class={inGutter}>
			<span aria-hidden="true">·</span>
			<ActorPlace {actor} /></span
		>{/if}
{/snippet}

<ChatHeader title={session.title} {subtitle}>
	{#snippet leading()}
		{#if subagent}
			<!-- Up, to the session it runs under, however it was reached. -->
			<BackLink
				href={routes.actorSession(subagent.parent_session_id)}
				label={above?.title ?? 'the session above'}
				iconOnly
				up
			/>
		{/if}
		<!-- Where it stands leads, as an agent's avatar leads its conversation. -->
		<span class="flex size-7 flex-none items-center justify-center">
			{#key session.status}
				<span class="flex" in:scale={{ start: 0.4, duration: motion.duration, easing: backOut }}
					><StatusIcon status={session.status} unread={session.unread} size={22} /></span
				>
			{/key}
		</span>
	{/snippet}
	{#snippet trailing()}
		{#if model}
			<span class={['flex-none text-xs whitespace-nowrap text-fg-muted max-sm:hidden', inGutter]}
				>{model}</span
			>
		{/if}
		{#if actions}
			<div class="flex flex-none gap-2">{@render actions()}</div>
		{/if}
	{/snippet}
</ChatHeader>
