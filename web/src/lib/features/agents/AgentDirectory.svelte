<!--
	Every agent on the Agents page, grouped under its team by title, pinned
	ones in place, and those on no team last under "No team". Each team's
	menu opens its skills, renames it or deletes it. With no teams, one plain list.
-->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import type { IntelligenceAgent, IntelligenceAgentTeam } from '$lib/protocol/types';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Menu from '$lib/ui/Menu.svelte';
	import AgentList from './AgentList.svelte';
	import SkillsDialog from './SkillsDialog.svelte';
	import TeamDialog from './TeamDialog.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents: agentsStore } = pageStores();

	interface Props {
		agents: IntelligenceAgent[];
		teams: IntelligenceAgentTeam[];
	}

	let { agents, teams }: Props = $props();

	let renaming = $state<IntelligenceAgentTeam | null>(null);
	let renameOpen = $state(false);
	let deleting = $state<IntelligenceAgentTeam | null>(null);
	let skillsTeam = $state<IntelligenceAgentTeam | null>(null);
	let skillsOpen = $state(false);

	const unteamed = $derived(agents.filter((a) => !teams.some((t) => t.id === a.team_id)));
</script>

{#snippet heading(title: string, team: IntelligenceAgentTeam | null)}
	<div class="mt-5 mb-1 flex items-center gap-2 px-2 first:mt-0">
		<h2 class="min-w-0 flex-1 truncate text-xs font-medium text-fg-subtle">{title}</h2>
		{#if team}
			<Menu
				items={[
					{
						label: 'Skills…',
						onSelect: () => {
							skillsTeam = team;
							skillsOpen = true;
						}
					},
					{
						label: 'Rename…',
						onSelect: () => {
							renaming = team;
							renameOpen = true;
						}
					},
					{ label: 'Delete…', danger: true, separated: true, onSelect: () => (deleting = team) }
				]}
			>
				{#snippet trigger(props)}
					<button
						class="inline-flex size-6 cursor-pointer items-center justify-center rounded-full text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
						type="button"
						aria-label="{team.title} actions"
						{...props}
					>
						<Ellipsis size={14} strokeWidth={1.75} />
					</button>
				{/snippet}
			</Menu>
		{/if}
	</div>
{/snippet}

{#if teams.length === 0}
	<AgentList {agents} selected={null} />
{:else}
	{#each teams as team (team.id)}
		{@const members = agents.filter((a) => a.team_id === team.id)}
		<section>
			{@render heading(team.title, team)}
			{#if members.length}
				<AgentList agents={members} selected={null} />
			{:else}
				<p class="px-2 py-1.5 text-fg-muted">No agents on this team yet.</p>
			{/if}
		</section>
	{/each}
	{#if unteamed.length}
		<section>
			{@render heading('No team', null)}
			<AgentList agents={unteamed} selected={null} />
		</section>
	{/if}
{/if}

<TeamDialog bind:open={renameOpen} team={renaming} />
<SkillsDialog bind:open={skillsOpen} team={skillsTeam} />
<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.title ?? ''}”?"
	description="The team’s agents won’t be deleted, but its memories and skills will."
	onconfirm={(team) => agentsStore.deleteTeam(team.id)}
/>
