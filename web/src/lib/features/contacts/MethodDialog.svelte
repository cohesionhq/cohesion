<!--
	Adds an email address or phone number to a contact, or changes one: what
	it says, then its type, which only an edit offers.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ContactMethod, ContactMethodKind } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { methodLabels } from './methods';

	const { contacts } = pageStores();

	interface Props {
		open?: boolean;
		workspaceId: number;
		contactId: number;
		/** The method to change; absent to add one of `kind`. */
		method?: ContactMethod | null;
		/** What a new method starts as. */
		kind?: ContactMethodKind;
	}

	let {
		open = $bindable(false),
		workspaceId,
		contactId,
		method = null,
		kind: initialKind = 'email'
	}: Props = $props();

	const id = $props.id();
	let kind = $state<ContactMethodKind>('email');
	let value = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	const kindOptions = (Object.keys(methodLabels) as ContactMethodKind[]).map((k) => ({
		value: k,
		label: methodLabels[k]
	}));

	// Each open starts from the method, or blank.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			kind = method?.kind ?? initialKind;
			value = method?.value ?? '';
			error = null;
		});
	});

	const canSave = $derived(!busy && value.trim() !== '');

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!canSave) return;
		busy = true;
		error = null;
		try {
			if (method) await contacts.updateMethod(workspaceId, method.id, kind, value.trim());
			else await contacts.addMethod(workspaceId, contactId, kind, value.trim());
			open = false;
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open
	title={method
		? `Edit ${methodLabels[method.kind].toLowerCase()}`
		: `Add ${methodLabels[initialKind].toLowerCase()}`}
>
	<form class="flex flex-col gap-4" onsubmit={save}>
		<Field label={methodLabels[kind]} for="{id}-value" {error}>
			<TextInput
				id="{id}-value"
				bind:value
				type={kind === 'phone' ? 'tel' : 'text'}
				inputmode={kind === 'email' ? 'email' : undefined}
				autocomplete="off"
				required
			/>
		</Field>

		{#if method}
			<Field label="Type" for="{id}-kind">
				<Select
					id="{id}-kind"
					class="w-full"
					bind:value={() => kind, (v) => (kind = v as ContactMethodKind)}
					options={kindOptions}
				/>
			</Field>
		{/if}

		<div class="flex justify-end gap-2">
			<Button onclick={() => (open = false)}>Cancel</Button>
			<Button variant="primary" type="submit" disabled={!canSave}>{method ? 'Save' : 'Add'}</Button>
		</div>
	</form>
</Dialog>
