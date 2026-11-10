<!--
	A subagent, as one quiet line in the transcript of the session it runs
	under, in place of the tool use that launched it: where it stands as its
	disc, what it is doing, its kind, and where it stands in words, opening its
	own thread. One still running in the background says so.
-->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { ActorSession } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { isGoing } from '$lib/stores/actors.svelte';
	import StatusIcon from '$lib/ui/StatusIcon.svelte';

	let { session }: { session: ActorSession } = $props();

	const kind = $derived(session.subagent?.agent_type ?? 'Subagent');
	const background = $derived((session.subagent?.background ?? false) && isGoing(session));
</script>

<a
	class="group flex h-6 min-w-0 items-center gap-1.5 rounded-sm text-xs text-fg-subtle no-underline hover:text-fg-muted hover:no-underline focus-visible:shadow-focus focus-visible:outline-none"
	href={routes.actorSession(session.id)}
>
	<StatusIcon status={session.status} size={12} />
	<span class="flex-none font-medium text-fg-muted group-hover:text-fg">{kind}</span>
	<span class="min-w-0 truncate">{session.title}</span>
	<span class="flex-none"
		>{vocabulary.actorSessionStatus(session.status)}{background ? ' · In the background' : ''}</span
	>
	<ChevronRight class="flex-none" size={12} strokeWidth={1.75} aria-hidden="true" />
</a>
