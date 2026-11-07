<!--
	Every actor session, the latest status change first, in one list rather
	than under each actor: one compact line each with where it stands, its
	title, marked with a dot while unread, then where its actor works (the
	folder, the whole path on hover), where it stands in words and since
	when, opening its transcript. A session whose actor no client offers
	any longer still shows, without the folder.
-->
<script lang="ts">
	import { scopeName } from './words';
	import { ago, nowSeconds } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { Actor, ActorSession } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';

	interface Props {
		actors: Actor[];
		/** Already latest status change first. */
		sessions: ActorSession[];
	}

	let { actors, sessions }: Props = $props();

	/** Ticks each minute, so "3m ago" stays true while the page is open. */
	let now = $state(nowSeconds());
	$effect(() => {
		const timer = setInterval(() => (now = nowSeconds()), 60_000);
		return () => clearInterval(timer);
	});

	const byId = $derived(new Map(actors.map((actor) => [actor.id, actor])));
</script>

<section aria-labelledby="actor-sessions-heading">
	<h2 id="actor-sessions-heading" class="border-b border-border pb-2 text-md font-semibold">
		Sessions
	</h2>
	{#if sessions.length === 0}
		<p class="pt-1 text-sm text-fg-subtle">No sessions yet.</p>
	{:else}
		<!-- One line a session, the whole row its link, so a long list stays short. -->
		<ul class="-mx-2 flex flex-col pt-1">
			{#each sessions as session (session.id)}
				{@const actor = byId.get(session.actor_id)}
				<li>
					<a
						class="flex h-9 items-center gap-2.5 rounded-md px-2 text-fg no-underline transition-colors hover:bg-bg-hover hover:no-underline"
						href={routes.actorSession(session.id)}
					>
						<StatusIcon status={session.status} />
						<span class="min-w-0 flex-1 truncate">{session.title}</span>
						{#if session.unread}
							<span class="sr-only">, unread</span>
							<span class="size-1.5 flex-none rounded-full bg-accent" aria-hidden="true"></span>
						{/if}
						{#if actor}
							<span class="max-w-[30%] flex-none truncate font-mono text-xs text-fg-muted"
								>{scopeName(actor)}</span
							>
						{/if}
						<span class="flex-none text-xs text-fg-subtle"
							>{vocabulary.actorSessionStatus(session.status)} · {ago(
								session.status_changed_at,
								now
							)}</span
						>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</section>
