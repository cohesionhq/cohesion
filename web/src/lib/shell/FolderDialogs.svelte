<!--
	The dialogs for making, renaming and deleting a document or task folder, which the sidebar's
	folder rows and the Documents and Tasks pages open through `folderDialogs`. Making a folder
	opens it; deleting the folder shown (or one holding it) goes to the Inbox.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { subtreeIds, tableIds } from '$lib/features/documents/folderTree';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { DocumentFolder, TaskFolder } from '$lib/protocol/types';
	import { folderDialogs, type FolderRequest } from '$lib/shell/folderDialogs.svelte';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	const { documents, tasks } = pageStores();

	const ws = $derived(workspaces.currentId);

	let request = $state<FolderRequest | null>(null);
	let naming = $state(false);
	let removing = $state(false);
	let title = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		const next = folderDialogs.pending;
		if (next === null) return;
		folderDialogs.pending = null;
		request = next;
		error = null;
		if (next.mode === 'delete') removing = true;
		else {
			title =
				next.mode !== 'rename' ? '' : next.kind === 'views' ? next.view.name : next.folder.title;
			naming = true;
		}
	});

	const dialogTitle = $derived.by(() => {
		if (request?.mode === 'rename')
			return request.kind === 'views' ? 'Rename view' : 'Rename folder';
		if (request?.mode === 'new' && request.kind === 'documents' && request.parent)
			return `New folder in “${request.parent.title}”`;
		return 'New folder';
	});

	const tableFolders = $derived(tableIds(documents.folders.value));
	/** What deleting does, in words: a table loses its columns and values. */
	const removeText = $derived.by(() => {
		if (request?.mode !== 'delete') return '';
		if (request.kind === 'views') return 'Tasks in this view won’t be deleted.';
		if (request.kind === 'tasks') return 'All tasks in this folder will also be deleted.';
		const target = request.folder;
		const inside = subtreeIds(documents.folders.value, target.id).filter((id) =>
			tableFolders.has(id)
		);
		const tablePart = tableFolders.has(target.id)
			? ' The table’s columns and values will be deleted too.'
			: inside.length > 0
				? ' The columns and values of the tables inside it will be deleted too.'
				: '';
		return `Folders inside this folder will also be deleted.${tablePart} The documents in them won’t be deleted and will move to the Inbox.`;
	});
	const removeTitle = $derived(
		request?.mode !== 'delete'
			? ''
			: `Delete “${request.kind === 'views' ? request.view.name : request.folder.title}”?`
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const name = title.trim();
		if (request === null || request.mode === 'delete' || ws === null || name === '') return;
		busy = true;
		try {
			if (request.mode === 'rename') {
				if (request.kind === 'views') await tasks.renameView(ws, request.view.id, name);
				else if (request.kind === 'documents')
					await documents.renameFolder(ws, request.folder.id, name);
				else await tasks.renameFolder(ws, request.folder.id, name);
			} else if (request.kind === 'documents') {
				const created = await documents.createFolder(ws, name, request.parent?.id ?? null);
				void goto(routes.documentFolder(ws, created.id));
			} else {
				const created = await tasks.createFolder(ws, name);
				void goto(`${routes.tasks(ws)}?view=${created.id}`);
			}
			naming = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}

	async function confirmRemove() {
		if (request === null || request.mode !== 'delete' || ws === null) return;
		busy = true;
		try {
			const shown = page.url.searchParams;
			if (request.kind === 'views') {
				await tasks.deleteView(ws, request.view.id);
			} else if (request.kind === 'documents') {
				const target: DocumentFolder = request.folder;
				const open = page.url.pathname === routes.documents(ws) ? Number(shown.get('folder')) : NaN;
				const goes = subtreeIds(documents.folders.value, target.id);
				await documents.deleteFolder(ws, target.id);
				if (goes.includes(open))
					void goto(`${routes.documents(ws)}?folder=inbox`, { replaceState: true });
			} else {
				const target: TaskFolder = request.folder;
				const open = page.url.pathname === routes.tasks(ws) ? shown.get('view') : null;
				await tasks.deleteFolder(ws, target.id);
				if (open === String(target.id))
					void goto(`${routes.tasks(ws)}?view=inbox`, { replaceState: true });
			}
			removing = false;
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open={naming} title={dialogTitle}>
	<form id="folder-form" onsubmit={submit}>
		<Field label="Title" for="folder-title" {error}>
			<TextInput
				id="folder-title"
				bind:value={title}
				autocomplete="off"
				oninput={() => (error = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (naming = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="folder-form"
			disabled={busy || title.trim() === ''}
		>
			{request?.mode === 'rename' ? 'Rename' : 'Create'}
		</Button>
	{/snippet}
</Dialog>

<Dialog bind:open={removing} title={removeTitle} description={removeText}>
	{#snippet footer()}
		<Button onclick={() => (removing = false)}>Cancel</Button>
		<Button variant="danger-quiet" disabled={busy} onclick={confirmRemove}>Delete</Button>
	{/snippet}
</Dialog>
