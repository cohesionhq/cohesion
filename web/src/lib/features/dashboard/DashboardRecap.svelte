<!--
	The Dashboard's recap: what the autonomous parts left for the user, in
	the order to deal with it. Actor sessions waiting on the user come first
	(amber), then sessions that finished a stretch of work and are still
	unread, then the agents that posted since their conversation was last
	read. Each row opens its item; nothing shows when there is nothing.
	Sessions still working follow in a section of their own, as rows of the
	same kind. A session's row has Resolve on hover or focus, as the
	sidebar's does, asking first while its actor is still at work.
-->
<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import { nowSeconds, relativeTime } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import type { ActorSession } from '$lib/protocol/types';
	import { isGoing } from '$lib/stores/actors.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { list } from '$lib/ui/list';
	import { sectionRow } from './section';

	const { actors, agents } = pageStores();

	/** Ticks each minute, so "3m" stays true while the page is open. */
	let now = $state(nowSeconds());
	$effect(() => {
		const timer = setInterval(() => (now = nowSeconds()), 60_000);
		return () => clearInterval(timer);
	});

	const waiting = $derived(actors.sessions.value.filter((s) => s.status === 'needsAttention'));
	const finished = $derived(
		actors.sessions.value.filter((s) => s.unread && s.status !== 'needsAttention')
	);
	const working = $derived(actors.sessions.value.filter((s) => s.status === 'working'));
	const unreadAgents = $derived(
		agents.list.value
			.filter((a) => a.agent_unread)
			.sort((a, b) => (b.last_message_at ?? 0) - (a.last_message_at ?? 0))
	);

	const heading = [list.heading, 'mb-0'];
	const row = sectionRow;

	/** The session waiting on the user's say before it is resolved. */
	let resolving = $state<ActorSession | null>(null);

	/** Resolves `session`, asking first while its actor is still at work. */
	function resolve(session: ActorSession) {
		if (isGoing(session)) resolving = session;
		else actors.resolve(session.id).catch((e) => toasts.error(e));
	}
</script>

{#snippet session(s: ActorSession, needsYou: boolean)}
	<li class="group/session relative">
		<!-- Lit while the pointer is anywhere on the row, its Resolve included. A touch screen, which always
		     shows Resolve, keeps the row's end clear of it. -->
		<a
			class={[row, 'group-hover/session:bg-bg-hover touch:pr-10']}
			href={routes.actorSession(s.id)}
		>
			<!-- The avatar's 20px slot, so every row's title starts at the same place. -->
			<span class="flex w-5 flex-none justify-center"
				><StatusIcon status={s.status} unread={s.unread} /></span
			>
			<span class="min-w-0 flex-1 truncate">{s.title}</span>
			<!-- Kept in place but hidden while Resolve shows, so the title never rewraps. -->
			<span
				class={[
					'flex-none text-xs group-focus-within/session:invisible group-hover/session:invisible touch:visible',
					needsYou ? 'text-warn' : 'text-fg-subtle'
				]}
				>{vocabulary.actorSessionStatus(s.status)} · {relativeTime(s.status_changed_at, now)}</span
			>
		</a>
		<!-- Beside the link, not in it: hover or focus shows it, a touch screen always does. -->
		<button
			type="button"
			class="absolute top-1/2 right-1.5 flex size-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md border-0 bg-transparent text-fg-subtle opacity-0 group-focus-within/session:opacity-100 group-hover/session:opacity-100 hover:bg-n3 hover:text-fg touch:opacity-100"
			aria-label="Resolve “{s.title}”"
			onclick={() => resolve(s)}
		>
			<Archive size={14} strokeWidth={1.75} />
		</button>
	</li>
{/snippet}

{#if waiting.length > 0 || finished.length > 0 || unreadAgents.length > 0}
	<section aria-labelledby="recap-heading">
		<h2 id="recap-heading" class={heading}>
			What’s new<span class={list.count}
				>{waiting.length + finished.length + unreadAgents.length}</span
			>
		</h2>
		<ul class="-mx-2 flex flex-col pt-1">
			{#each waiting as s (s.id)}
				{@render session(s, true)}
			{/each}
			{#each finished as s (s.id)}
				{@render session(s, false)}
			{/each}
			{#each unreadAgents as a (a.id)}
				<li>
					<a class={row} href={routes.agent(a.id)}>
						<AgentAvatar
							name={a.agent_name}
							color={a.agent_color}
							emoji={a.agent_emoji}
							size="xs"
						/>
						<span class="min-w-0 flex-1 truncate">{a.agent_name}</span>
						<span class="flex-none text-xs text-fg-subtle">
							New message{a.last_message_at ? ` · ${relativeTime(a.last_message_at, now)}` : ''}
						</span>
					</a>
				</li>
			{/each}
		</ul>
	</section>
{/if}

{#if working.length > 0}
	<section aria-labelledby="working-heading">
		<h2 id="working-heading" class={heading}>
			Working<span class={list.count}>{working.length}</span>
		</h2>
		<ul class="-mx-2 flex flex-col pt-1">
			{#each working as s (s.id)}
				{@render session(s, false)}
			{/each}
		</ul>
	</section>
{/if}

<ConfirmDialog
	subject={resolving}
	onclose={() => (resolving = null)}
	title="Resolve “{resolving?.title ?? ''}”?"
	description="The actor will stop what it’s doing. This session will be removed from your list. To pick it back up, send a message."
	confirmLabel="Resolve"
	onconfirm={(session) => actors.resolve(session.id)}
/>
