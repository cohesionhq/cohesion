<!--
	Names a new team, or renames one. A title the daemon refuses (empty, or
	another team's) shows under the field.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import type { IntelligenceAgentTeam } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { agents } = pageStores();

	interface Props {
		open: boolean;
		/** The team to rename; a new one when absent. */
		team?: IntelligenceAgentTeam | null;
		/** Called with the team once it is created or renamed. */
		onsaved?: (team: IntelligenceAgentTeam) => void;
	}

	let { open = $bindable(), team = null, onsaved }: Props = $props();

	let title = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (open) {
			title = team?.title ?? '';
			error = null;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (title.trim() === '') return;
		busy = true;
		try {
			const saved = team ? await agents.renameTeam(team.id, title) : await agents.createTeam(title);
			open = false;
			onsaved?.(saved);
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title={team ? 'Rename team' : 'New team'}>
	<form id="agent-team-form" onsubmit={submit}>
		<Field label="Title" for="agent-team-title" {error}>
			<TextInput
				id="agent-team-title"
				bind:value={title}
				autocomplete="off"
				oninput={() => (error = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="agent-team-form"
			disabled={busy || title.trim() === ''}
		>
			{team ? 'Rename' : 'Create'}
		</Button>
	{/snippet}
</Dialog>
