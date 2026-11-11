<!--
	Writes a new skill for an agent or a team, or changes one: its name, when
	to use it, its prompt, and whether the agent may use it itself. A field
	the daemon refuses (an empty name or prompt, a name taken in its scope)
	shows its error under it.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import type { AgentSkill, AgentSkillScope } from '$lib/protocol/types';
	import { skills } from '$lib/stores/skills.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open: boolean;
		/** Whose a new skill is; a skill being changed keeps its own. */
		scope: AgentSkillScope;
		/** Who may use it, as the switch names them: an agent's name, or "Agents" for a team. */
		users: string;
		/** The skill to change; a new one when absent. */
		skill?: AgentSkill | null;
	}

	let { open = $bindable(), scope, users, skill = null }: Props = $props();

	const id = $props.id();
	let name = $state('');
	let description = $state('');
	let prompt = $state('');
	let agentCanUse = $state(true);
	let errors = $state<{ name?: string; prompt?: string }>({});
	let busy = $state(false);

	$effect(() => {
		if (!open) return;
		name = skill?.name ?? '';
		description = skill?.description ?? '';
		prompt = skill?.prompt ?? '';
		agentCanUse = skill?.agent_can_use ?? true;
		errors = {};
	});

	const ready = $derived(name.trim() !== '' && prompt.trim() !== '');

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!ready) return;
		busy = true;
		const fields = { name, description, prompt, agent_can_use: agentCanUse };
		try {
			if (skill) await skills.update(skill.id, fields);
			else await skills.create(scope, fields);
			open = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed' && failure.field === 'prompt')
				errors = { prompt: failure.message };
			else if (failure.code === 'validationFailed') errors = { name: failure.message };
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title={skill ? 'Edit skill' : 'New skill'} size="wide">
	<form id="{id}-form" class="flex flex-col gap-4" onsubmit={submit}>
		<Field label="Name" for="{id}-name" error={errors.name}>
			<TextInput
				id="{id}-name"
				bind:value={name}
				autocomplete="off"
				placeholder="Weekly review"
				oninput={() => (errors = { ...errors, name: undefined })}
			/>
		</Field>
		<Field
			label="Description (optional)"
			for="{id}-description"
			help="When to use it. The agent reads this to decide."
		>
			<TextInput
				id="{id}-description"
				bind:value={description}
				autocomplete="off"
				placeholder="When I ask how the week went"
			/>
		</Field>
		<Field label="Prompt" for="{id}-prompt" error={errors.prompt}>
			<Textarea
				id="{id}-prompt"
				bind:value={prompt}
				rows={8}
				placeholder="What the agent should do…"
				oninput={() => (errors = { ...errors, prompt: undefined })}
			/>
		</Field>
		<div class="flex items-start gap-3">
			<Switch id="{id}-agent" checked={agentCanUse} onchange={(on) => (agentCanUse = on)} />
			<label class="flex cursor-pointer flex-col gap-0.5" for="{id}-agent">
				<span class="font-medium">{users} can use this</span>
				<span class="text-xs text-fg-subtle">
					When it’s off, only you can use it, by typing / in the message box.
				</span>
			</label>
		</div>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="{id}-form" disabled={busy || !ready}>
			{skill ? 'Save' : 'Create'}
		</Button>
	{/snippet}
</Dialog>
