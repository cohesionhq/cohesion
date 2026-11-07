<!--
	Where an actor session stands, in its header's trailing edge: a word or two
	about it, in a quiet pill, led by the status's icon.
-->
<script lang="ts">
	import type { ActorSessionStatus } from '$lib/protocol/types';
	import StatusIcon from './StatusIcon.svelte';

	interface Props {
		status: ActorSessionStatus;
		label: string;
	}

	let { status, label }: Props = $props();
</script>

<span
	class={[
		'flex h-6 max-w-full min-w-0 flex-none items-center gap-1.5 rounded-full px-2 text-xs',
		// Waiting on the user, it takes the amber every place gives that state.
		status === 'needsAttention' ? 'bg-warn/12 font-medium text-warn' : 'bg-bg-hover text-fg-muted'
	]}
>
	<StatusIcon {status} size={14} /><span class="truncate">{label}</span>
</span>
