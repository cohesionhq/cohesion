<!--
	A dialog asking for one name: a new calendar, a new store, or a calendar's
	new title. `onsubmit` saves it; a validation error shows under the field,
	any other error as a toast.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		title: string;
		/** The name the field starts with. */
		initial?: string;
		/** The button that saves: "Create", "Rename". */
		submitLabel: string;
		onsubmit: (name: string) => Promise<unknown>;
	}

	let { open = $bindable(false), title, initial = '', submitLabel, onsubmit }: Props = $props();

	let name = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);
	const id = $props.id();

	$effect.pre(() => {
		if (open) {
			name = initial;
			error = null;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (trimmed === '') return;
		busy = true;
		try {
			await onsubmit(trimmed);
			open = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open {title}>
	<form id="{id}-form" onsubmit={submit}>
		<Field label="Name" for="{id}-name" {error}>
			<TextInput
				id="{id}-name"
				bind:value={name}
				autocomplete="off"
				oninput={() => (error = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="{id}-form" disabled={busy || name.trim() === ''}>
			{submitLabel}
		</Button>
	{/snippet}
</Dialog>
