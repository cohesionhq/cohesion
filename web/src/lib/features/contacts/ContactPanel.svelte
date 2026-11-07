<!--
	A contact's panel (`ItemPanel`): its photo, if it has one, then its email
	addresses and phone numbers, each followed from its link and changed or
	removed from its ⋯, with buttons to add either, then its notes. The title
	row's ⋯ edits its names and notes, adds, changes or removes its photo, or
	deletes it.
-->
<script lang="ts">
	import BookUser from '@lucide/svelte/icons/book-user';
	import Mail from '@lucide/svelte/icons/mail';
	import Phone from '@lucide/svelte/icons/phone';
	import Plus from '@lucide/svelte/icons/plus';
	import type { Contact, ContactMethod, ContactMethodKind } from '$lib/protocol/types';
	import { fullName } from '$lib/stores/contacts.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { imageFiles, PHOTO_SIDE, prepareImage } from '$lib/ui/images';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import Photo from '$lib/ui/Photo.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import ContactDialog from './ContactDialog.svelte';
	import MethodDialog from './MethodDialog.svelte';
	import { methodHref, methodLabels } from './methods';

	const { contacts } = pageStores();

	interface Props {
		open?: boolean;
		workspaceId: number;
		contact: Contact | null;
	}

	let { open = $bindable(false), workspaceId, contact: given }: Props = $props();

	/** The last contact given, kept while the panel fades out after a delete. */
	let kept: Contact | null = null;
	const contact = $derived.by(() => {
		if (given) kept = given;
		return kept;
	});

	const icons = { email: Mail, phone: Phone };

	let editing = $state(false);
	let deleting = $state(false);
	let methodOpen = $state(false);
	let method = $state<ContactMethod | null>(null);
	let methodKind = $state<ContactMethodKind>('email');
	let removing = $state<ContactMethod | null>(null);

	function addMethod(kind: ContactMethodKind) {
		method = null;
		methodKind = kind;
		methodOpen = true;
	}

	function editMethod(m: ContactMethod) {
		method = m;
		methodOpen = true;
	}

	let picker = $state<HTMLInputElement>();
	let removingPhoto = $state(false);

	async function onpick(event: Event & { currentTarget: HTMLInputElement }) {
		const [file] = imageFiles(event.currentTarget.files);
		event.currentTarget.value = '';
		if (!file || !contact) return;
		try {
			await contacts.setPhoto(workspaceId, contact.id, await prepareImage(file, PHOTO_SIDE));
		} catch (error) {
			toasts.error(error);
		}
	}
</script>

{#if contact}
	<ItemPanel bind:open title={fullName(contact)} icon={BookUser} kind="Contact">
		{#snippet actions()}
			<RowMenu
				label="More for {fullName(contact)}"
				inDialog
				items={[
					{ label: 'Edit…', onSelect: () => (editing = true) },
					...(contact.photo.kind === 'image'
						? [
								{ label: 'Change photo…', onSelect: () => picker?.click() },
								{ label: 'Remove photo…', onSelect: () => (removingPhoto = true) }
							]
						: [{ label: 'Add photo…', onSelect: () => picker?.click() }]),
					{ label: 'Delete…', danger: true, separated: true, onSelect: () => (deleting = true) }
				]}
			/>
		{/snippet}
		{#snippet details()}
			{#if contact.photo.kind === 'image'}
				<dt class="sr-only">Photo</dt>
				<dd class="col-span-2 mb-3"><Photo photo={contact.photo} size="xl" /></dd>
			{/if}
			{#each contact.methods as m (m.id)}
				<ItemDetail label={methodLabels[m.kind]} icon={icons[m.kind]}>
					<span class="flex min-w-0 flex-1 items-center justify-between gap-2">
						<a class="min-w-0 break-all" href={methodHref(m.kind, m.value)}>{m.value}</a>
						<RowMenu
							label="More for {m.value}"
							inDialog
							items={[
								{ label: 'Edit…', onSelect: () => editMethod(m) },
								{ label: 'Remove…', danger: true, onSelect: () => (removing = m) }
							]}
						/>
					</span>
				</ItemDetail>
			{/each}
			<dt class="sr-only">Add</dt>
			<dd class="col-start-2 -ml-3 flex flex-wrap gap-x-1 [&>button]:text-fg-muted">
				<Button onclick={() => addMethod('email')}
					><Plus size={15} strokeWidth={1.75} aria-hidden="true" />Add email…</Button
				>
				<Button onclick={() => addMethod('phone')}
					><Plus size={15} strokeWidth={1.75} aria-hidden="true" />Add phone…</Button
				>
			</dd>
		{/snippet}
		{#if contact.notes.trim()}
			<div class="text-md"><Markdown source={contact.notes} /></div>
		{:else}
			<p class="text-fg-subtle">No notes.</p>
		{/if}
	</ItemPanel>

	<input class="hidden" type="file" accept="image/*" bind:this={picker} onchange={onpick} />

	<ContactDialog bind:open={editing} {workspaceId} {contact} />

	<MethodDialog
		bind:open={methodOpen}
		{workspaceId}
		contactId={contact.id}
		{method}
		kind={methodKind}
	/>

	<ConfirmDialog
		bind:open={deleting}
		title="Delete {fullName(contact)}?"
		description="This contact’s photo, email addresses, phone numbers and notes will also be deleted."
		onconfirm={async () => {
			await contacts.remove(workspaceId, contact.id);
			open = false;
		}}
	/>

	<ConfirmDialog
		bind:open={removingPhoto}
		title="Remove photo?"
		description="This can’t be undone."
		confirmLabel="Remove"
		onconfirm={() => contacts.setPhoto(workspaceId, contact.id, null)}
	/>

	<ConfirmDialog
		subject={removing}
		onclose={() => (removing = null)}
		title="Remove {removing?.value ?? ''}?"
		description="This can’t be undone."
		confirmLabel="Remove"
		onconfirm={(m) => contacts.removeMethod(workspaceId, m.id)}
	/>
{/if}
