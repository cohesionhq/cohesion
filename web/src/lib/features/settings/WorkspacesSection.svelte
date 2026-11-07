<script lang="ts">
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
			<ul class={list.rows}>
				{#each workspaces.list.value as workspace (workspace.id)}
					<li class={list.row}>
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
								<a class="text-fg" href={routes.workspace(workspace.id)}>{workspace.title}</a>
								{#if workspace.id === workspaces.currentId}
									<span
										class="rounded-full bg-bg-hover px-1.5 py-px text-2xs font-medium text-fg-muted"
										>Current</span
									>
								{/if}
							</div>
							<div class={list.actions}>
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
