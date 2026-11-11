<!--
	The skills of an agent and of its team, or of a team alone: each under
	whose it is, with when to use it and whether only the user can use it.
	Each section adds one; each row edits or deletes its skill.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type {
		AgentSkill,
		AgentSkillScope,
		IntelligenceAgent,
		IntelligenceAgentTeam
	} from '$lib/protocol/types';
	import { inScope, skills } from '$lib/stores/skills.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { list } from '$lib/ui/list';
	import SkillDialog from './SkillDialog.svelte';

	interface Props {
		open?: boolean;
		/** The agent whose skills show first; none for a team's alone. */
		agent?: IntelligenceAgent | null;
		/** The team whose skills show: the agent's, or the one the dialog is for. */
		team: IntelligenceAgentTeam | null;
	}

	let { open = $bindable(false), agent = null, team }: Props = $props();

	$effect(() => {
		if (open) untrack(() => void skills.load());
	});

	/** What the skill dialog edits: a new skill in a scope, or one to change. */
	let editing = $state<{
		scope: AgentSkillScope;
		users: string;
		skill: AgentSkill | null;
	} | null>(null);
	let editOpen = $state(false);
	let deleting = $state<AgentSkill | null>(null);

	function edit(scope: AgentSkillScope, users: string, skill: AgentSkill | null) {
		editing = { scope, users, skill };
		editOpen = true;
	}
</script>

{#snippet group(title: string, description: string, scope: AgentSkillScope, users: string)}
	{@const items = skills.list.value.filter((s) => inScope(s, scope))}
	<section class="flex flex-col">
		<div class="flex items-start gap-3">
			<div class="min-w-0 flex-1">
				<h3 class="font-medium">{title}</h3>
				<p class="text-xs text-fg-subtle">{description}</p>
			</div>
			<Button onclick={() => edit(scope, users, null)}>New skill…</Button>
		</div>
		<ul class={list.rows}>
			{#each items as skill (skill.id)}
				<li class={list.row}>
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<p class="wrap-anywhere">
							<span class="font-medium">{skill.name}</span>
							{#if !skill.agent_can_use}
								<span class="ml-1 text-xs text-fg-subtle">Only you can use this</span>
							{/if}
						</p>
						{#if skill.description}
							<p class="text-xs wrap-anywhere text-fg-muted">{skill.description}</p>
						{/if}
					</div>
					<div class={list.actions}>
						<Button variant="quiet" onclick={() => edit(scope, users, skill)}>Edit…</Button>
						<Button variant="danger-quiet" onclick={() => (deleting = skill)}>Delete…</Button>
					</div>
				</li>
			{:else}
				<li class={list.empty}>No skills yet.</li>
			{/each}
		</ul>
	</section>
{/snippet}

<Dialog bind:open title="Skills" size="wide">
	<div class="flex max-h-[60vh] flex-col gap-5 overflow-y-auto">
		{#if skills.list.error && !skills.list.loaded}
			<p class="text-fg-subtle">{skills.list.error.message}</p>
		{:else if skills.list.loaded}
			{#if agent}
				{@render group(
					agent.agent_name,
					`Only ${agent.agent_name} has these.`,
					{ kind: 'agent', agent_id: agent.id },
					agent.agent_name
				)}
			{/if}
			{#if team}
				{@render group(
					team.title,
					`Every agent on “${team.title}” has these.`,
					{ kind: 'team', team_id: team.id },
					'Agents'
				)}
			{/if}
		{/if}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Close</Button>
	{/snippet}
</Dialog>

{#if editing}
	<SkillDialog
		bind:open={editOpen}
		scope={editing.scope}
		users={editing.users}
		skill={editing.skill}
	/>
{/if}
<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.name ?? ''}”?"
	description="This can’t be undone."
	confirmLabel="Delete"
	onconfirm={(skill) => skills.delete(skill.id)}
/>
