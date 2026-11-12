<!--
	Creates an agent, or edits one: its name, emoji, color, role and team (once there are teams),
	then, under More options, its directive, personality and verbosity, and, for an agent being
	edited, the model it talks on in place of the agent default.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type {
		IntelligenceAgent,
		IntelligenceAgentColor,
		IntelligenceAgentPersonality,
		IntelligenceAgentVerbosity
	} from '$lib/protocol/types';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { type AgentChanges } from '$lib/stores/agents.svelte';
	import { intelligence, modelGroups } from '$lib/stores/settings/intelligence.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Disclosure from '$lib/ui/Disclosure.svelte';
	import EmojiPicker from '$lib/ui/EmojiPicker.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import AgentColorPicker from './AgentColorPicker.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents } = pageStores();

	interface Props {
		open?: boolean;
		/** The agent to edit; absent to create one. */
		agent?: IntelligenceAgent | null;
		onsaved?: (agent: IntelligenceAgent) => void;
	}

	let { open = $bindable(false), agent = null, onsaved }: Props = $props();

	const id = $props.id();
	const NO_TEAM = 'none';
	let name = $state('');
	let emoji = $state('');
	let color = $state<IntelligenceAgentColor>('blue');
	let personality = $state<IntelligenceAgentPersonality>('balanced');
	let verbosity = $state<IntelligenceAgentVerbosity>('brief');
	let role = $state('');
	let directive = $state('');
	/** Whether More options is open. */
	let more = $state(false);
	/** The team's id, or `NO_TEAM`, as the select holds it. */
	let team = $state(NO_TEAM);
	const teamOptions = $derived([
		...agents.teams.value.map((t) => ({ value: String(t.id), label: t.title })),
		{ value: NO_TEAM, label: 'No team' }
	]);
	const teamId = $derived(team === NO_TEAM ? null : Number(team));
	const DEFAULT_MODEL = 'default';
	/** The endpoint model's id, or `DEFAULT_MODEL`, as the select holds it. */
	let model = $state(DEFAULT_MODEL);
	const modelOptions = $derived.by(() => {
		const endpoints = intelligence.endpoints.value;
		const tierModel = intelligence.defaults.value.find((d) => d.tier === 'agent');
		const endpoint = endpoints.find((e) =>
			e.models.some((m) => m.id === tierModel?.endpoint_model_id)
		);
		const current = endpoint?.models.find((m) => m.id === tierModel?.endpoint_model_id);
		const label =
			current && endpoint ? `Default (${current.model_name} · ${endpoint.name})` : 'Default';
		return [{ value: DEFAULT_MODEL, label }, ...modelGroups(endpoints)];
	});
	const modelId = $derived(model === DEFAULT_MODEL ? null : Number(model));
	let busy = $state(false);
	let error = $state<{ message: string; field: string | null } | null>(null);

	// Each open starts from the agent, or blank.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			name = agent?.agent_name ?? '';
			emoji = agent?.agent_emoji ?? '';
			color = agent?.agent_color ?? 'blue';
			personality = agent?.agent_personality ?? 'balanced';
			verbosity = agent?.agent_verbosity ?? 'brief';
			role = agent?.agent_role ?? '';
			directive = agent?.agent_directive ?? '';
			team = agent?.team_id != null ? String(agent.team_id) : NO_TEAM;
			model = agent?.endpoint_model_id != null ? String(agent.endpoint_model_id) : DEFAULT_MODEL;
			more = false;
			error = null;
		});
	});

	// The model picker lists the endpoints' models while the dialog is open.
	$effect(() => {
		if (!open || !agent) return;
		untrack(() => void intelligence.load());
		return () => {
			intelligence.endpoints.release();
			intelligence.defaults.release();
		};
	});

	// A new agent left unnamed gets a name from the daemon; an existing one keeps needing one.
	const canSave = $derived(!busy && (!agent || name.trim() !== ''));

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!canSave) return;
		busy = true;
		error = null;
		try {
			let saved: IntelligenceAgent;
			if (agent) {
				const changes: AgentChanges = {};
				if (name.trim() !== agent.agent_name) changes.name = name.trim();
				if (color !== agent.agent_color) changes.color = color;
				if (emoji.trim() !== (agent.agent_emoji ?? '')) changes.emoji = emoji.trim();
				if (personality !== agent.agent_personality) changes.personality = personality;
				if (verbosity !== agent.agent_verbosity) changes.verbosity = verbosity;
				if (role.trim() !== agent.agent_role) changes.role = role.trim();
				if (directive.trim() !== agent.agent_directive) changes.directive = directive.trim();
				saved = await agents.update(agent.id, changes);
				if (teamId !== agent.team_id) saved = await agents.setTeam(agent.id, teamId);
				if (modelId !== agent.endpoint_model_id) saved = await agents.setModel(agent.id, modelId);
			} else {
				saved = await agents.create({
					name,
					color,
					emoji: emoji.trim() || null,
					personality,
					verbosity,
					role: role.trim(),
					directive: directive.trim(),
					team_id: teamId
				});
			}
			open = false;
			onsaved?.(saved);
		} catch (e) {
			const failure = toProtocolError(e);
			error = { message: failure.message, field: failure.field ?? null };
		} finally {
			busy = false;
		}
	}

	const fieldError = (field: string) => (error?.field === field ? error.message : null);
</script>

<Dialog bind:open title={agent ? `Edit ${agent.agent_name}` : 'New agent'}>
	<form class="flex flex-col gap-4" onsubmit={save}>
		<Field
			label={agent ? 'Name' : 'Name (optional)'}
			for="{id}-name"
			error={fieldError('emoji') ?? fieldError('name')}
		>
			<div class="flex gap-2">
				<EmojiPicker bind:value={emoji} />
				<TextInput
					id="{id}-name"
					bind:value={name}
					placeholder={agent ? agent.agent_name : 'One is picked for you'}
					autocomplete="off"
				/>
			</div>
		</Field>

		<AgentColorPicker bind:value={color} />

		<Field label="Role" for="{id}-role">
			<TextInput
				id="{id}-role"
				bind:value={role}
				placeholder="What is this agent for?"
				autocomplete="off"
			/>
		</Field>

		{#if agents.teams.value.length > 0}
			<Field label="Team" for="{id}-team">
				<Select id="{id}-team" bind:value={team} options={teamOptions} />
			</Field>
		{/if}

		<Disclosure label="More options" bind:open={more} class="gap-4 pt-4">
			<Field label="Directive" for="{id}-directive">
				<Textarea
					id="{id}-directive"
					bind:value={directive}
					placeholder="What rules should it always follow?"
				/>
			</Field>

			<div class="flex gap-4 max-sm:flex-col">
				<Field label="Personality" for="{id}-personality">
					<Select
						id="{id}-personality"
						bind:value={personality}
						options={options(vocabulary.value.agent_personalities)}
					/>
				</Field>
				<Field label="Verbosity" for="{id}-verbosity">
					<Select
						id="{id}-verbosity"
						bind:value={verbosity}
						options={options(vocabulary.value.agent_verbosities)}
					/>
				</Field>
			</div>

			{#if agent}
				<Field label="Model" for="{id}-model">
					<Select id="{id}-model" bind:value={model} options={modelOptions} />
				</Field>
			{/if}
		</Disclosure>

		{#if error && error.field !== 'name' && error.field !== 'emoji'}
			<p class="text-xs text-danger" role="alert">{error.message}</p>
		{/if}

		<div class="flex justify-end gap-2">
			<Button onclick={() => (open = false)}>Cancel</Button>
			<Button variant="primary" type="submit" disabled={!canSave}
				>{agent ? 'Save' : 'Create'}</Button
			>
		</div>
	</form>
</Dialog>
