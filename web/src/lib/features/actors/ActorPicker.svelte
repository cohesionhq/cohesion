<!--
	Which actor takes a new session, as a menu bar chip naming it that opens
	onto a list of them, each with the device it runs on. An offline actor
	shows but can't be picked.
-->
<script lang="ts">
	import type { Actor } from '$lib/protocol/types';
	import Choices from '$lib/ui/Choices.svelte';
	import MenuBarItem from '$lib/ui/MenuBarItem.svelte';
	import { scopeName } from './words';

	interface Props {
		actors: Actor[];
		/** Whether client `id` is online, as the clients list says. */
		online: (id: number) => boolean;
		actorId: number | null;
		onpick: (actor: Actor) => void;
	}

	let { actors, online, actorId, onpick }: Props = $props();

	const actor = $derived(actors.find((a) => a.id === actorId) ?? null);
	const label = $derived(actor ? scopeName(actor) : 'Choose an actor');
</script>

<MenuBarItem {label} name="Actor">
	{#snippet children(close)}
		<Choices
			label="Actor"
			choices={actors.map((candidate) => {
				const reachable = online(candidate.client_id);
				return {
					id: candidate.id,
					label: scopeName(candidate),
					detail: reachable ? candidate.title : `${candidate.title}, offline`,
					disabled: !reachable
				};
			})}
			checked={actorId}
			onpick={(id) => {
				const picked = actors.find((a) => a.id === id);
				if (picked) onpick(picked);
				close();
			}}
		/>
	{/snippet}
</MenuBarItem>
