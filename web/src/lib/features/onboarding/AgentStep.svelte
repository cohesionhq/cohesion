<!--
	The flow's third page: makes a first agent from the fields that matter
	most (an emoji and name, a color and a role), previewed large beside the
	title. Everything else is left to the agent's own settings.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import type { IntelligenceAgent, IntelligenceAgentColor } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmojiPicker from '$lib/ui/EmojiPicker.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import AgentColorPicker from '../agents/AgentColorPicker.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		/** The agent made here, once it is. */
		created: IntelligenceAgent | null;
		oncreated: (agent: IntelligenceAgent) => void;
		onback: () => void;
		onnext: () => void;
	}

	let { created, oncreated, onback, onnext }: Props = $props();

	const { agents } = pageStores();
	const id = $props.id();
	let name = $state('');
	let emoji = $state('');
	let color = $state<IntelligenceAgentColor>('blue');
	let role = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			oncreated(
				await agents.create({ name, color, emoji: emoji.trim() || null, role: role.trim() })
			);
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<StepPage
	eyebrow="Step 3 of 3"
	title={created ? `Say hi to ${created.agent_name}.` : 'Meet your first agent.'}
	description={created
		? `${created.agent_name} can chat with you, write documents and work through tasks.`
		: 'Agents chat with you, write documents and work through tasks. Give this one a face and a job.'}
	{onback}
	onskip={created ? undefined : onnext}
>
	{#snippet art()}
		<div class="scale-[1.6] -rotate-3 p-4">
			<AgentAvatar
				name={created?.agent_name ?? (name.trim() || '?')}
				color={created?.agent_color ?? color}
				emoji={created ? created.agent_emoji : emoji.trim() || null}
				size="lg"
			/>
		</div>
	{/snippet}

	{#if !created}
		<form id="{id}-form" class="flex flex-col gap-4" onsubmit={create}>
			<Field label="Name (optional)" for="{id}-name" {error}>
				<div class="flex gap-2">
					<EmojiPicker bind:value={emoji} />
					<TextInput
						id="{id}-name"
						bind:value={name}
						placeholder="One is picked for you"
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
		</form>
	{/if}

	{#snippet action()}
		{#if created}
			<Button variant="primary" onclick={onnext}>Continue</Button>
		{:else}
			<Button variant="primary" type="submit" form="{id}-form" disabled={busy}>
				{busy ? 'Creating…' : 'Create agent'}
			</Button>
		{/if}
	{/snippet}
</StepPage>
