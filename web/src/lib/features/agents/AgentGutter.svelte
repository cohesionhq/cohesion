<!--
	The column beside an agent's conversation: its context size, role and how many sessions it follows, its agenda
	as a row of dots opening a card (`AgendaOverview`), the actor sessions
	not yet resolved that it handed work and follows, each led by where it stands
	and naming its actor, with Stop following and Resolve on hover or focus, then its cues.
-->
<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import BellRing from '@lucide/svelte/icons/bell-ring';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import { flip } from 'svelte/animate';
	import { backOut } from 'svelte/easing';
	import { fade, fly, scale } from 'svelte/transition';
	import type {
		Actor,
		ActorSession,
		AgentAgendaItem,
		IntelligenceAgent,
		IntelligenceAgentCue
	} from '$lib/protocol/types';
	import { nowSeconds, relativeTime } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import { cueTitle } from '$lib/stores/agents.svelte';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import GutterGroup from '$lib/ui/GutterGroup.svelte';
	import { motion } from '$lib/ui/motion';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import AgendaOverview from './AgendaOverview.svelte';
	import ContextTokens from './ContextTokens.svelte';

	interface Props {
		agent: IntelligenceAgent;
		cues: IntelligenceAgentCue[];
		/** The agent's agenda, in order, finished items included. */
		agenda: AgentAgendaItem[];
		/** Every actor, which names each session's. */
		actors: Actor[];
		/** The sessions not yet resolved that the agent follows. */
		sessions: ActorSession[];
		onresolve: (session: ActorSession) => void;
		/** Has the agent stop following `session`, so it no longer wakes when the session stops. */
		onunfollow: (session: ActorSession) => void;
	}

	let { agent, cues, agenda, actors, sessions, onresolve, onunfollow }: Props = $props();

	const actorTitle = (id: number) => actors.find((actor) => actor.id === id)?.title;
	// A row's Stop following and Resolve, on the canvas their wrapper lays under them.
	const actionClass =
		'flex size-6 cursor-pointer items-center justify-center rounded-md border-0 bg-transparent text-fg-subtle hover:bg-bg-hover hover:text-fg';
	// Each session names its actor only when they differ; the same name under every row says nothing.
	const namesActors = $derived(
		sessions.length === 1 ||
			sessions.some((s) => actorTitle(s.actor_id) !== actorTitle(sessions[0].actor_id))
	);
</script>

<!-- Two lines, as the session column's card is: how much context the conversation fills (its name
     is the header's), then its role and, when it follows any, how many sessions, which the sidebar's eye counts. -->
<GutterCard>
	<p class="truncate text-sm text-fg">
		<span class="tabular-nums"><ContextTokens tokens={agent.context_tokens} /></span> tokens in context
	</p>
	<p class="truncate text-xs text-fg-muted">
		{[agent.agent_role, agent.following_count > 0 && `Monitoring ${agent.following_count}`]
			.filter(Boolean)
			.join(' · ')}
	</p>
</GutterCard>

{#if agenda.length > 0}
	<!-- Slides shut when the agent clears its agenda, and open when it sets out a new one. -->
	<GutterGroup slides>
		<AgendaOverview {agenda} />
	</GutterGroup>
{/if}

{#if sessions.length > 0}
	<GutterGroup>
		<a
			class="flex items-center text-xs font-medium text-fg-subtle no-underline hover:text-fg hover:no-underline"
			href={routes.actors()}>Monitoring</a
		>
		<div class="flex flex-col gap-px">
			{#each sessions as session (session.id)}
				{@const actor = namesActors ? actorTitle(session.actor_id) : undefined}
				<div
					class="group/session relative"
					in:fly={{ y: 8, duration: motion.duration }}
					out:fade={{ duration: motion.duration }}
					animate:flip={{ duration: motion.duration }}
				>
					<!-- A touch screen, which always shows the actions, keeps the row's end clear of them. -->
					<a
						class="flex cursor-pointer items-start gap-2 py-1 text-left text-sm text-fg-muted no-underline hover:text-fg hover:no-underline touch:pr-13"
						href={routes.actorSession(session.id)}
					>
						<span class="sr-only">{vocabulary.actorSessionStatus(session.status)}, </span>
						<!-- Centred on the title's line, where the main sidebar draws a session's icon. -->
						<span class="flex h-5.25 w-4 flex-none items-center justify-center">
							{#key session.status}
								<span
									class="flex"
									in:scale={{ start: 0.4, duration: motion.duration, easing: backOut }}
									><StatusIcon status={session.status} unread={session.unread} size={16} /></span
								>
							{/key}
						</span>
						<!-- What the sidebar can't say: where it stands and since when, then its actor when they differ. -->
						<span class="flex min-w-0 flex-col">
							<span class="line-clamp-2 text-balance">{session.title}</span>
							<span
								class={[
									// The size the Dashboard gives the same words.
									'truncate text-xs',
									session.status === 'needsAttention' ? 'text-warn' : 'text-fg-subtle'
								]}
								>{[
									vocabulary.actorSessionStatus(session.status),
									relativeTime(session.status_changed_at, nowSeconds()),
									actor
								]
									.filter(Boolean)
									.join(' · ')}</span
							>
						</span>
					</a>
					<!-- Beside the link, not in it: hover or focus shows them, a touch screen always does. They lie over
				     the row's end on the column's canvas, faded in on their left, so showing them never rewraps a title. -->
					<div
						class="absolute top-1/2 right-0 flex -translate-y-1/2 items-center gap-0.5 bg-canvas opacity-0 group-focus-within/session:opacity-100 group-hover/session:opacity-100 before:absolute before:inset-y-0 before:right-full before:w-3 before:bg-linear-to-r before:from-transparent before:to-canvas touch:opacity-100"
					>
						<button
							type="button"
							class={actionClass}
							aria-label="Stop following “{session.title}”"
							onclick={() => onunfollow(session)}
						>
							<EyeOff size={14} strokeWidth={1.75} />
						</button>
						<button
							type="button"
							class={actionClass}
							aria-label="Resolve “{session.title}”"
							onclick={() => onresolve(session)}
						>
							<Archive size={14} strokeWidth={1.75} />
						</button>
					</div>
				</div>
			{/each}
		</div>
	</GutterGroup>
{/if}

{#if cues.length > 0}
	<GutterGroup>
		<h2 class="text-xs font-medium text-fg-subtle">Cues</h2>
		<ul class="flex flex-col gap-1 text-xs text-fg-muted">
			{#each cues as cue (cue.id)}
				<li class="flex items-baseline gap-1.5">
					<!-- Sits on the first line's baseline at a capital's height, nudged down 1px so it reads as centred. -->
					<BellRing
						class="size-[0.85em] flex-none translate-y-px text-fg-muted"
						strokeWidth={1.75}
						aria-hidden="true"
					/>
					<span class="min-w-0 wrap-anywhere">{cueTitle(cue.name, cue.hook)}</span>
				</li>
			{/each}
		</ul>
	</GutterGroup>
{/if}
