<!--
	Names a database: a new one, or a new title for one. The daemon's refusal
	shows under the field.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Database } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		/** The database to rename; a new one when null. */
		database?: Database | null;
		onsubmit: (title: string) => Promise<unknown>;
	}

	let { open = $bindable(false), database = null, onsubmit }: Props = $props();

	let title = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			title = database?.title ?? '';
			error = null;
		});
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const next = title.trim();
		if (next === '') return;
		busy = true;
		try {
			await onsubmit(next);
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

<Dialog bind:open title={database ? 'Rename database' : 'New database'}>
	<form id="database-form" onsubmit={submit}>
		<Field label="Title" for="database-title" {error}>
			<TextInput
				id="database-title"
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
			form="database-form"
			disabled={busy || title.trim() === ''}
		>
			{database ? 'Rename' : 'Create'}
		</Button>
	{/snippet}
</Dialog>
