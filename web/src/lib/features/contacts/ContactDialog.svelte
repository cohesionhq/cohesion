<!--
	Creates a contact, or edits one's names and notes. A new contact opens on
	its first and last names, an email and a phone, with its middle name and
	notes under More options; editing opens on the names, with the same fold.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Contact, ContactMethodKind } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Disclosure from '$lib/ui/Disclosure.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';

	const { contacts } = pageStores();

	interface Props {
		open?: boolean;
		workspaceId: number;
		/** The contact to edit; absent to create one. */
		contact?: Contact | null;
		onsaved?: (contact: Contact) => void;
	}

	let { open = $bindable(false), workspaceId, contact = null, onsaved }: Props = $props();

	const id = $props.id();
	let first = $state('');
	let middle = $state('');
	let last = $state('');
	let notes = $state('');
	let email = $state('');
	let phone = $state('');
	/** Whether More options is open. */
	let more = $state(false);
	let busy = $state(false);
	let error = $state<{ message: string; field: string | null } | null>(null);

	// Each open starts from the contact, or blank.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			first = contact?.first_name ?? '';
			middle = contact?.middle_name ?? '';
			last = contact?.last_name ?? '';
			notes = contact?.notes ?? '';
			email = '';
			phone = '';
			more = false;
			error = null;
		});
	});

	const canSave = $derived(!busy && first.trim() !== '' && last.trim() !== '');

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!canSave) return;
		busy = true;
		error = null;
		const fields = {
			first_name: first.trim(),
			middle_name: middle.trim(),
			last_name: last.trim(),
			notes: notes.trim()
		};
		try {
			let saved: Contact;
			if (contact) {
				saved = await contacts.update(workspaceId, contact.id, fields);
			} else {
				const methods: { kind: ContactMethodKind; value: string }[] = [];
				if (email.trim()) methods.push({ kind: 'email', value: email.trim() });
				if (phone.trim()) methods.push({ kind: 'phone', value: phone.trim() });
				saved = await contacts.create(workspaceId, fields, methods);
			}
			open = false;
			onsaved?.(saved);
		} catch (e) {
			const failure = toProtocolError(e);
			error = { message: failure.message, field: failure.field ?? null };
			if (error.field === 'middle_name' || error.field === 'notes') more = true;
		} finally {
			busy = false;
		}
	}

	const fieldError = (field: string) => (error?.field === field ? error.message : null);
	const shownElsewhere = ['first_name', 'middle_name', 'last_name'];
</script>

<Dialog bind:open title={contact ? 'Edit contact' : 'New contact'}>
	<form class="flex flex-col gap-4" onsubmit={save}>
		<div class="flex gap-4 max-sm:flex-col">
			<Field label="First name" for="{id}-first" error={fieldError('first_name')}>
				<TextInput id="{id}-first" bind:value={first} autocomplete="off" required />
			</Field>
			<Field label="Last name" for="{id}-last" error={fieldError('last_name')}>
				<TextInput id="{id}-last" bind:value={last} autocomplete="off" required />
			</Field>
		</div>

		{#if !contact}
			<Field label="Email (optional)" for="{id}-email">
				<TextInput id="{id}-email" bind:value={email} inputmode="email" autocomplete="off" />
			</Field>
			<Field label="Phone (optional)" for="{id}-phone">
				<TextInput id="{id}-phone" bind:value={phone} type="tel" autocomplete="off" />
			</Field>
		{/if}

		<Disclosure label="More options" bind:open={more} class="gap-4 pt-4">
			<Field label="Middle name (optional)" for="{id}-middle" error={fieldError('middle_name')}>
				<TextInput id="{id}-middle" bind:value={middle} autocomplete="off" />
			</Field>
			<Field label="Notes (optional)" for="{id}-notes">
				<Textarea id="{id}-notes" bind:value={notes} placeholder="Write about them…" />
			</Field>
		</Disclosure>

		{#if error && !shownElsewhere.includes(error.field ?? '')}
			<p class="text-xs text-danger" role="alert">{error.message}</p>
		{/if}

		<div class="flex justify-end gap-2">
			<Button onclick={() => (open = false)}>Cancel</Button>
			<Button variant="primary" type="submit" disabled={!canSave}
				>{contact ? 'Save' : 'Create'}</Button
			>
		</div>
	</form>
</Dialog>
