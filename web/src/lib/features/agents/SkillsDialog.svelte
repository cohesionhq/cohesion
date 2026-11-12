<!--
	The skills of an agent and of its team, or of a team alone, one list at a
	time: the agent's own (Personal) or its team's, switched by a segmented
	control when there are both. Each row shows a skill's name, whether only
	the user can use it, and one line of when to use it. New skill adds one
	to the list showing; each row edits or deletes its skill.
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
	import Segmented from '$lib/ui/Segmented.svelte';
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

	interface View {
		id: AgentSkillScope['kind'];
		label: string;
		description: string;
		scope: AgentSkillScope;
		/** Who can use a skill here, for the skill dialog. */
		users: string;
	}

	/** Each list the control switches between: whose skills, and the words for it. */
	const views = $derived.by(() => {
		const all: View[] = [];
		if (agent)
			all.push({
				id: 'agent',
				label: 'Personal',
				description: `Only ${agent.agent_name} has these.`,
				scope: { kind: 'agent', agent_id: agent.id },
				users: agent.agent_name
			});
		if (team)
			all.push({
				id: 'team',
				label: 'Team',
				description: `Every agent on “${team.title}” has these.`,
				scope: { kind: 'team', team_id: team.id },
				users: 'Agents'
			});
		return all;
	});

	const inView = (v: View) => skills.list.value.filter((s) => inScope(s, v.scope));

	let chosen = $state<View['id']>('agent');
	const view = $derived(views.find((v) => v.id === chosen) ?? views[0]);
	const items = $derived(view ? inView(view) : []);

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

<Dialog bind:open title="Skills" size="wide">
	{#if skills.list.error && !skills.list.loaded}
		<p class="text-fg-subtle">{skills.list.error.message}</p>
	{:else if skills.list.loaded && view}
		<div class="flex items-end gap-3 border-b border-border pb-3">
			<div class="flex min-w-0 flex-1 flex-col items-start gap-2">
				{#if views.length > 1}
					<Segmented
						label="Whose skills"
						segments={views.map((v) => ({ id: v.id, label: v.label, count: inView(v).length }))}
						value={view.id}
						onchange={(id) => (chosen = id as View['id'])}
					/>
				{/if}
				<p class="text-xs text-fg-subtle">{view.description}</p>
			</div>
			<Button onclick={() => edit(view.scope, view.users, null)}>New skill…</Button>
		</div>
		<ul class={[list.rows, 'max-h-[55vh] overflow-y-auto']}>
			{#each items as skill (skill.id)}
				<li class="group/row flex items-center gap-3 border-border py-3 not-first:border-t">
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<p class="flex min-w-0 items-center gap-2">
							<span class="truncate font-semibold">{skill.name}</span>
							{#if !skill.agent_can_use}
								<span
									class="inline-flex h-4.5 flex-none items-center rounded-full bg-current-item px-1.75 text-2xs leading-none font-medium text-fg-muted"
									>Only you can use this</span
								>
							{/if}
						</p>
						{#if skill.description}
							<p class="truncate text-xs text-fg-muted">{skill.description}</p>
						{/if}
					</div>
					<div class={[list.actions, list.reveal]}>
						<Button variant="quiet" onclick={() => edit(view.scope, view.users, skill)}>
							Edit…
						</Button>
						<Button variant="danger-quiet" onclick={() => (deleting = skill)}>Delete…</Button>
					</div>
				</li>
			{:else}
				<li class={list.empty}>No skills yet.</li>
			{/each}
		</ul>
	{/if}
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
