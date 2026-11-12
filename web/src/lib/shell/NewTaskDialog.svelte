<!--
	The new-task dialog: the Tasks page's composer (`AddTaskRow`) in a modal, with the workspace it
	files into chosen first. It starts in the current workspace and takes any other; the composer
	chooses the folder, priority, size, tags, days and description from that workspace's own.
-->
<script lang="ts">
	import AddTaskRow from '$lib/features/tasks/AddTaskRow.svelte';
	import { send } from '$lib/protocol/api';
	import type { Tag, TaskFolder } from '$lib/protocol/types';
	import type { NewTaskDetails } from '$lib/stores/tasks.svelte';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { newTaskDialog } from './newTaskDialog.svelte';

	let workspaceId = $state<string>('');
	let folders = $state<TaskFolder[]>([]);
	let tags = $state<Tag[]>([]);
	let element = $state<HTMLInputElement>();

	const options = $derived(
		workspaces.list.value.map((w) => ({ value: String(w.id), label: w.title }))
	);
	const chosen = $derived(workspaces.list.value.find((w) => String(w.id) === workspaceId) ?? null);

	// Each open starts in the current workspace.
	$effect(() => {
		if (!newTaskDialog.open) return;
		const current = workspaces.currentId ?? workspaces.list.value[0]?.id;
		workspaceId = current === undefined ? '' : String(current);
	});

	// The chosen workspace's folders and tags; a newer choice drops an older answer.
	$effect(() => {
		const id = chosen?.id;
		folders = [];
		tags = [];
		if (id === undefined || !newTaskDialog.open) return;
		let current = true;
		void Promise.all([
			send('listTaskFolders', { workspace_id: id }),
			send('listTags', { workspace_id: id })
		]).then(
			([f, t]) => {
				if (!current) return;
				folders = f;
				tags = t;
			},
			(error) => current && toasts.error(error)
		);
		return () => (current = false);
	});

	// The title field is where typing starts.
	$effect(() => {
		if (newTaskDialog.open && element) element.focus();
	});

	async function create(title: string, details: NewTaskDetails) {
		if (chosen === null) return;
		await send('createTask', { workspace_id: chosen.id, title, ...details });
		toasts.show(`Added to ${chosen.title}.`);
		newTaskDialog.open = false;
	}

	async function createTag(name: string): Promise<Tag> {
		const tag = await send('createTag', { workspace_id: chosen!.id, name });
		tags = [...tags, tag];
		return tag;
	}
</script>

<Dialog bind:open={newTaskDialog.open} title="New task" size="wide">
	<div class="flex flex-col gap-3">
		{#if options.length > 1}
			<Field label="Workspace" for="new-task-workspace">
				<Select id="new-task-workspace" bind:value={workspaceId} {options} />
			</Field>
		{/if}
		{#if chosen !== null}
			{#key chosen.id}
				<AddTaskRow
					bind:element
					viewName={chosen.title}
					{folders}
					{tags}
					folderId={null}
					tagIds={[]}
					oncreate={create}
					oncreatetag={createTag}
					ondown={() => {}}
				/>
			{/key}
		{/if}
	</div>
</Dialog>
