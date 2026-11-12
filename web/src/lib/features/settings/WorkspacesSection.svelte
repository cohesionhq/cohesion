<!--
	The workspaces, in the order the switcher, sidebar and ⌘1–⌘9 use:
	renames one in place, deletes one, creates one, and reorders them by
	their handles or with Alt+↑/↓ on a row.
-->
<script lang="ts">
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import { untrack } from 'svelte';
	import { resolve } from '$app/paths';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Workspace } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import WorkspaceBadge from '$lib/shell/WorkspaceBadge.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { list } from '$lib/ui/list';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import Section from './Section.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	let editingId = $state<number | null>(null);
	let editTitle = $state('');
	let editError = $state<string | null>(null);
	let newTitle = $state('');
	let newError = $state<string | null>(null);
	let creating = $state(false);
	let deleting = $state<Workspace | null>(null);
	let editInput = $state<HTMLInputElement>();

	$effect(() => {
		editInput?.focus();
		editInput?.select();
	});

	$effect(() => {
		untrack(() => void workspaces.load());
	});

	function startRename(workspace: Workspace) {
		editingId = workspace.id;
		editTitle = workspace.title;
		editError = null;
	}

	async function saveRename(workspace: Workspace) {
		// Enter saves and closes the field; the blur that follows finds it closed.
		if (editingId !== workspace.id) return;
		const title = editTitle.trim();
		if (title === workspace.title) {
			editingId = null;
			return;
		}
		// The daemon renames to an empty title without complaint, so refuse it here.
		if (title === '') {
			editError = 'A title is required.';
			return;
		}
		try {
			await workspaces.rename(workspace.id, title);
			editingId = null;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') editError = e.message;
			else toasts.error(e);
		}
	}

	function renameKey(event: KeyboardEvent, workspace: Workspace) {
		if (event.key === 'Enter') {
			event.preventDefault();
			void saveRename(workspace);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			editingId = null;
		}
	}

	async function create(event: SubmitEvent) {
		event.preventDefault();
		creating = true;
		newError = null;
		try {
			await workspaces.create(newTitle.trim());
			newTitle = '';
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') newError = e.message;
			else toasts.error(e);
		} finally {
			creating = false;
		}
	}

	const order = new Reorderable(
		() => workspaces.list.value,
		(workspace) => workspace.id,
		(ids) => workspaces.reorder(ids).catch((e) => toasts.error(e))
	);

	function onHandleDrag(event: DragEvent, workspace: Workspace) {
		order.start(event, workspace, workspace.title);
		// The whole row follows the pointer, not only the handle.
		const row = (event.currentTarget as HTMLElement).closest('li')!;
		const { left, top } = row.getBoundingClientRect();
		event.dataTransfer!.setDragImage(row, event.clientX - left, event.clientY - top);
	}

	async function remove(workspace: Workspace) {
		const wasCurrent = workspace.id === workspaces.currentId;
		await workspaces.remove(workspace.id);
		// The sidebar follows the last workspace; land on another one, or the picker.
		if (wasCurrent) {
			const next = workspaces.pick();
			if (next === null) void page.goto(resolve('/'));
			else workspaces.currentId = next;
		}
	}
</script>

<Section
	title="Your workspaces"
	description="Each has its own journal, documents, tasks and resources."
>
	<ResourceState resource={workspaces.list}>
		{#snippet loaded()}
			<ul class={list.rows} aria-label="Your workspaces" {...order.list}>
				{#each order.items as workspace, index (workspace.id)}
					<!-- Alt+↑/↓ from the row's link or menu moves it. -->
					<li
						class={['group/row', list.row, order.dragged === workspace.id && 'opacity-50']}
						{...order.target(index)}
						onkeydown={(event) => editingId === null && order.key(event, index)}
					>
						<span
							class={[
								'-mx-1 inline-flex h-8 w-4 flex-none cursor-grab items-center justify-center text-fg-subtle hover:text-fg',
								list.reveal
							]}
							draggable="true"
							aria-hidden="true"
							ondragstart={(e) => onHandleDrag(e, workspace)}
							ondragend={() => order.end()}
						>
							<GripVertical size={14} strokeWidth={1.75} />
						</span>
						{#if editingId === workspace.id}
							<div class="min-w-0 flex-1">
								<TextInput
									bind:value={editTitle}
									aria-label="Workspace title"
									autocomplete="off"
									bind:element={editInput}
									onkeydown={(event) => renameKey(event, workspace)}
									onblur={() => void saveRename(workspace)}
								/>
								{#if editError}<p class={list.error}>{editError}</p>{/if}
							</div>
						{:else}
							<div class={list.main}>
								<WorkspaceBadge {workspace} />
								<a
									class="text-fg"
									href={routes.workspace(workspace.id)}
									aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown">{workspace.title}</a
								>
							</div>
							<!-- Where every list keeps a row's state: at its trailing edge. -->
							<div class={list.actions}>
								{#if workspace.id === workspaces.currentId}
									<span class="px-1 text-xs text-fg-subtle">Current</span>
								{/if}
								<RowMenu
									label="More for {workspace.title}"
									items={[
										{ label: 'Rename', onSelect: () => startRename(workspace) },
										{
											label: 'Delete…',
											danger: true,
											separated: true,
											onSelect: () => (deleting = workspace)
										}
									]}
								/>
							</div>
						{/if}
					</li>
				{:else}
					<li class={list.empty}>No workspaces yet.</li>
				{/each}
			</ul>

			<form class="mt-4 flex items-start gap-2" onsubmit={create}>
				<div class="max-w-90 flex-1">
					<TextInput
						bind:value={newTitle}
						placeholder="New workspace title"
						aria-label="New workspace title"
						autocomplete="off"
					/>
					{#if newError}<p class={list.error}>{newError}</p>{/if}
				</div>
				<Button variant="primary" type="submit" disabled={creating || newTitle.trim() === ''}>
					Create
				</Button>
			</form>
		{/snippet}
	</ResourceState>
</Section>

<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.title ?? ''}”?"
	description="All journal entries, documents, tasks and resources in this workspace will also be deleted."
	onconfirm={remove}
/>
