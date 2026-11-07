<!--
	The column beside an agent's conversation: its context size, the actor sessions
	not yet resolved that it handed work and follows, each led by where it stands
	and naming its actor, then its cues.
-->
<script lang="ts">
	import BellRing from '@lucide/svelte/icons/bell-ring';
	import { flip } from 'svelte/animate';
	import { backOut } from 'svelte/easing';
	import { fade, fly, scale } from 'svelte/transition';
	import type {
		Actor,
		ActorSession,
		IntelligenceAgent,
		IntelligenceAgentCue
	} from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import { cueTitle } from '$lib/stores/agents.svelte';
	import GutterCard from '$lib/ui/GutterCard.svelte';
	import GutterGroup from '$lib/ui/GutterGroup.svelte';
	import { motion } from '$lib/ui/motion';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';
	import ContextTokens from './ContextTokens.svelte';

	interface Props {
		agent: IntelligenceAgent;
		cues: IntelligenceAgentCue[];
		/** Every actor, which names each session's. */
		actors: Actor[];
		/** The sessions not yet resolved that the agent follows. */
		sessions: ActorSession[];
	}

	let { agent, cues, actors, sessions }: Props = $props();

	const actorTitle = (id: number) => actors.find((actor) => actor.id === id)?.title;
</script>

<GutterCard>
	<p class="flex items-baseline gap-1.5">
		<!-- Muted until there is a count, so the placeholder never reads as one. -->
		<span
			class={[
				'text-lg leading-tight font-semibold tabular-nums transition-colors',
				agent.context_tokens === null && 'text-fg-muted'
			]}
		>
			<ContextTokens tokens={agent.context_tokens} />
		</span>
		<span class="text-xs text-fg-subtle">tokens</span>
	</p>
</GutterCard>

{#if sessions.length > 0}
	<GutterGroup>
		<a
			class="flex items-center text-xs font-medium text-fg-subtle no-underline hover:text-fg hover:no-underline"
			href={routes.actors()}>Monitoring</a
		>
		<div class="flex flex-col gap-px">
			{#each sessions as session (session.id)}
				{@const actor = actorTitle(session.actor_id)}
				<a
					class="flex cursor-pointer items-start gap-2 py-1 text-left text-xs text-fg-muted no-underline hover:text-fg hover:no-underline"
					href={routes.actorSession(session.id)}
					in:fly={{ y: 8, duration: motion.duration }}
					out:fade={{ duration: motion.duration }}
					animate:flip={{ duration: motion.duration }}
				>
					<span class="sr-only">{vocabulary.actorSessionStatus(session.status)}, </span>
					<!-- Centred on the title's line, where the main sidebar draws a session's icon. -->
					<span class="flex h-5 w-4 flex-none items-center justify-center">
						{#key session.status}
							<span
								class="flex"
								in:scale={{ start: 0.4, duration: motion.duration, easing: backOut }}
								><StatusIcon status={session.status} unread={session.unread} size={14} /></span
							>
						{/key}
					</span>
					<span class="flex min-w-0 flex-col">
						<span class="truncate">{session.title}</span>
						{#if actor}
							<span class="truncate text-2xs text-fg-subtle">{actor}</span>
						{/if}
					</span>
				</a>
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
