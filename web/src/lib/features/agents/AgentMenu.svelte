<!--
	The conversation's overflow menu: pin, edit, the team it's on, the cues,
	memories, runs, and a confirmed delete.
-->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import type { IntelligenceAgentTeam } from '$lib/protocol/types';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Menu from '$lib/ui/Menu.svelte';

	interface Props {
		name: string;
		/** Whether the agent is pinned to the top of the sidebar. */
		pinned: boolean;
		onpin: () => void;
		onedit: () => void;
		/** Every team, by title, each one the agent can move to. */
		teams: IntelligenceAgentTeam[];
		/** The team the agent is on; null for none. */
		teamId: number | null;
		/** Moves the agent onto a team, or onto none. */
		onmove: (teamId: number | null) => void;
		/** Opens the naming of a new team for the agent to move onto. */
		onnewteam: () => void;
		/** Opens the cues; absent when the agent has none. */
		oncues?: () => void;
		onmemories: () => void;
		onruns: () => void;
		ondelete: () => Promise<unknown>;
	}

	let {
		name,
		pinned,
		onpin,
		onedit,
		teams,
		teamId,
		onmove,
		onnewteam,
		oncues,
		onmemories,
		onruns,
		ondelete
	}: Props = $props();

	let confirming = $state(false);
</script>

<Menu
	items={[
		{ label: pinned ? 'Unpin' : 'Pin', onSelect: onpin },
		{ label: 'Edit…', onSelect: onedit },
		{
			label: 'Move to',
			items: [
				...teams.map((team) => ({
					id: team.id,
					label: team.title,
					checked: team.id === teamId,
					onSelect: () => onmove(team.id)
				})),
				{
					id: 'none',
					label: 'No team',
					checked: teamId === null,
					separated: teams.length > 0,
					onSelect: () => onmove(null)
				},
				{ id: 'new', label: 'New team…', separated: true, onSelect: onnewteam }
			]
		},
		{
			label: 'Cues…',
			separated: true,
			disabled: !oncues,
			detail: oncues ? undefined : 'No cues yet.',
			onSelect: () => oncues?.()
		},
		{ label: 'Memories…', onSelect: onmemories },
		{ label: 'Runs…', onSelect: onruns },
		{ label: 'Delete…', danger: true, separated: true, onSelect: () => (confirming = true) }
	]}
>
	{#snippet trigger(props)}
		<button
			class="inline-flex size-7.5 cursor-pointer items-center justify-center rounded-full text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
			type="button"
			aria-label="Agent actions"
			{...props}
		>
			<Ellipsis size={16} strokeWidth={1.75} />
		</button>
	{/snippet}
</Menu>

<ConfirmDialog
	bind:open={confirming}
	title="Delete {name}?"
	description="{name}’s conversation and runs will also be deleted. This can’t be undone."
	onconfirm={ondelete}
/>
