<!--
	The Dashboard's recap: what the autonomous parts left for the user, in
	the order to deal with it. Actor sessions waiting on the user come first
	(amber), then sessions that finished a stretch of work and are still
	unread, then the agents that posted since their conversation was last
	read. Each row opens its item; nothing shows when there is nothing.
	Sessions still working follow in a section of their own, as rows of the
	same kind.
-->
<script lang="ts">
	import { nowSeconds, relativeTime } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
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
</script>

{#snippet session(s: (typeof waiting)[number], needsYou: boolean)}
	<li>
		<a class={row} href={routes.actorSession(s.id)}>
			<!-- The avatar's 20px slot, so every row's title starts at the same place. -->
			<span class="flex w-5 flex-none justify-center"
				><StatusIcon status={s.status} unread={s.unread} /></span
			>
			<span class="min-w-0 flex-1 truncate">{s.title}</span>
			<span class={['flex-none text-xs', needsYou ? 'text-warn' : 'text-fg-subtle']}
				>{vocabulary.actorSessionStatus(s.status)} · {relativeTime(s.status_changed_at, now)}</span
			>
		</a>
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
