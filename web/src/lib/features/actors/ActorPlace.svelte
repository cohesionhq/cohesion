<!--
	What an actor is, the device it runs on and where it works, in one line:
	"Claude Code · Mac mini · cohesion". Where it works is the scope's last
	part, the whole of it on hover. The device is its client's name, left
	out until the clients list has it.
-->
<script lang="ts">
	import type { Actor } from '$lib/protocol/types';
	import { clients } from '$lib/stores/settings/clients.svelte';
	import { scopeName } from './words';

	interface Props {
		actor: Actor;
		/** Classes for the path. */
		pathClass?: string;
	}

	let { actor, pathClass }: Props = $props();

	const device = $derived(clients.name(actor.client_id));
</script>

{actor.product}{#if device}&nbsp;· {device}{/if}&nbsp;·
<span class={pathClass}>{scopeName(actor)}</span>
