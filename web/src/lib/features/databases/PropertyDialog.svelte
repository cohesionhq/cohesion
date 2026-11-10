<!--
	A property's name and type: a new property takes both, renaming one its
	name, changing its type the type, with a warning that values that don't fit
	the new type are cleared. The daemon's refusal shows under its field.
-->
<script lang="ts" module>
	import type { DatabaseDataType, DatabaseProperty } from '$lib/protocol/types';

	/** What the dialog is doing. */
	export type PropertyEdit =
		| { mode: 'new' }
		| { mode: 'rename'; property: DatabaseProperty }
		| { mode: 'type'; property: DatabaseProperty };

	export interface PropertyFields {
		name: string;
		data_type: DatabaseDataType;
	}
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		edit: PropertyEdit | null;
		onsubmit: (fields: PropertyFields) => Promise<unknown>;
	}

	let { open = $bindable(false), edit, onsubmit }: Props = $props();

	let name = $state('');
	let dataType = $state<string>('text');
	let error = $state<string | null>(null);
	/** Whether the refusal is of the name, so it shows under that field rather than the type. */
	let nameError = $state(false);
	let busy = $state(false);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			const property = edit && edit.mode !== 'new' ? edit.property : null;
			name = property?.name ?? '';
			dataType = property?.data_type ?? 'text';
			error = null;
		});
	});

	const mode = $derived(edit?.mode ?? 'new');
	const title = $derived(
		edit?.mode === 'rename'
			? 'Rename property'
			: edit?.mode === 'type'
				? `Change the type of “${edit.property.name}”?`
				: 'New property'
	);
	const unchanged = $derived(
		edit?.mode === 'rename'
			? name.trim() === edit.property.name
			: edit?.mode === 'type' && dataType === edit.property.data_type
	);
	// Text reads every value, so only another type clears any.
	const description = $derived(
		mode === 'type' && dataType !== 'text'
			? 'Values that don’t fit the new type will be cleared. This can’t be undone.'
			: undefined
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (mode !== 'type' && name.trim() === '') return;
		busy = true;
		try {
			await onsubmit({ name: name.trim(), data_type: dataType as DatabaseDataType });
			open = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') {
				error = failure.message;
				nameError = mode === 'rename' || (mode === 'new' && failure.field === 'name');
			} else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open {title} {description}>
	<form class="flex flex-col gap-4" id="property-form" onsubmit={submit}>
		{#if mode !== 'type'}
			<Field label="Name" for="property-name" error={nameError ? error : null}>
				<TextInput
					id="property-name"
					bind:value={name}
					autocomplete="off"
					oninput={() => (error = null)}
				/>
			</Field>
		{/if}
		{#if mode !== 'rename'}
			<Field label="Type" for="property-type" error={nameError ? null : error}>
				<Select
					id="property-type"
					bind:value={dataType}
					options={options(vocabulary.value.database_data_types)}
					onchange={() => (error = null)}
				/>
			</Field>
		{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="property-form"
			disabled={busy || unchanged || (mode !== 'type' && name.trim() === '')}
		>
			{mode === 'rename' ? 'Rename' : mode === 'type' ? 'Change type' : 'Create'}
		</Button>
	{/snippet}
</Dialog>
