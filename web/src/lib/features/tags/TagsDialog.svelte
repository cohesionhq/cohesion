<!--
	Manages a workspace's tags, as a list of names that read as text
	until hovered or focused: renames one in place (Enter or leaving the field
	saves), reorders them by their handles or with Alt+↑/↓, sorts them A–Z
	from the footer's menu, deletes them, and adds one from the "Add a tag"
	row that ends the list.
-->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash from '@lucide/svelte/icons/trash-2';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Tag } from '$lib/protocol/types';
	import type { TagsStore } from '$lib/stores/tags.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open: boolean;
		store: TagsStore;
		workspaceId: number;
	}

	let { open = $bindable(), store, workspaceId }: Props = $props();

	let name = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function create(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (trimmed === '' || busy) return;
		busy = true;
		try {
			await store.create(workspaceId, trimmed);
			name = '';
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}

	// Each row's field, as typed; a row not in it shows the tag's own name.
	let drafts = $state<Record<number, string>>({});
	let rowErrors = $state<Record<number, string>>({});

	async function rename(tag: Tag) {
		const next = drafts[tag.id]?.trim();
		if (next === undefined) return;
		if (next === '' || next === tag.name) {
			delete drafts[tag.id];
			return;
		}
		try {
			await store.rename(workspaceId, tag.id, next);
			delete drafts[tag.id];
			delete rowErrors[tag.id];
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') rowErrors[tag.id] = failure.message;
			else toasts.error(failure);
		}
	}

	const order = new Reorderable(
		() => store.list.value,
		(tag) => tag.id,
		(ids) => store.reorder(workspaceId, ids).catch((e) => toasts.error(e))
	);

	function onHandleDrag(event: DragEvent, tag: Tag) {
		order.start(event, tag, tag.name);
		// The whole row follows the pointer, not only the handle.
		const row = (event.currentTarget as HTMLElement).closest('li')!;
		const { left, top } = row.getBoundingClientRect();
		event.dataTransfer!.setDragImage(row, event.clientX - left, event.clientY - top);
	}

	let deleting = $state<Tag | null>(null);

	const more: MenuItem[] = [
		{
			label: 'Sort A–Z',
			onSelect: () => store.sortByName(workspaceId).catch((e) => toasts.error(e))
		}
	];

	// A row's leading icon: a tag's grip or the add row's plus.
	const handle = 'inline-flex h-8 w-4 flex-none items-center justify-center text-fg-subtle';
	// A name reads as text until it's hovered or focused.
	const field = 'border-transparent bg-transparent hover:border-border';
</script>

<Dialog bind:open title="Tags" onclosed={() => ((name = ''), (error = null), (drafts = {}))}>
	<ul class="flex flex-col" aria-label="Tags" {...order.list}>
		{#each order.items as tag, index (tag.id)}
			<li
				class={[
					'group/tag flex items-start gap-1 border-b border-border py-1',
					order.dragged === tag.id && 'opacity-50'
				]}
				{...order.target(index)}
			>
				<span
					class={[
						handle,
						'cursor-grab opacity-0 transition-opacity group-focus-within/tag:opacity-100 group-hover/tag:opacity-100 hover:text-fg touch:opacity-100'
					]}
					draggable="true"
					aria-hidden="true"
					ondragstart={(e) => onHandleDrag(e, tag)}
					ondragend={() => order.end()}
				>
					<GripVertical size={14} strokeWidth={1.75} />
				</span>
				<div class="min-w-0 flex-1">
					<TextInput
						class={field}
						value={drafts[tag.id] ?? tag.name}
						aria-label="Name of {tag.name}"
						aria-invalid={rowErrors[tag.id] !== undefined}
						autocomplete="off"
						oninput={(event) => {
							drafts[tag.id] = event.currentTarget.value;
							delete rowErrors[tag.id];
						}}
						onblur={() => rename(tag)}
						aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
						onkeydown={(event) => {
							order.key(event, index);
							if (event.key === 'Enter') {
								event.preventDefault();
								void rename(tag);
							}
						}}
					/>
					{#if rowErrors[tag.id]}
						<p class="mx-2 mt-1 mb-1 text-xs text-danger">{rowErrors[tag.id]}</p>
					{/if}
				</div>
				<button
					class="inline-flex size-8 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle opacity-0 transition-opacity group-focus-within/tag:opacity-100 group-hover/tag:opacity-100 hover:bg-bg-hover hover:text-danger focus-visible:opacity-100 touch:opacity-100"
					type="button"
					aria-label="Delete {tag.name}"
					onclick={() => (deleting = tag)}
				>
					<Trash size={15} strokeWidth={1.75} />
				</button>
			</li>
		{/each}
	</ul>
	<form class="flex items-start gap-1 py-1" onsubmit={create}>
		<span class={handle} aria-hidden="true">
			<Plus size={14} strokeWidth={1.75} />
		</span>
		<div class="min-w-0 flex-1">
			<TextInput
				class={field}
				bind:value={name}
				placeholder="Add a tag"
				aria-label="Add a tag"
				aria-invalid={error !== null}
				autocomplete="off"
				oninput={() => (error = null)}
			/>
			{#if error}
				<p class="mx-2 mt-1 mb-1 text-xs text-danger">{error}</p>
			{/if}
		</div>
		<!-- Lines the field up with the names above, past their delete buttons. -->
		<span class="size-8 flex-none"></span>
	</form>
	{#snippet footer()}
		<span class="mr-auto">
			<Menu items={more} align="start" inDialog>
				{#snippet trigger(props)}
					<button
						class="inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg"
						type="button"
						aria-label="More for tags"
						{...props}
					>
						<Ellipsis size={16} strokeWidth={1.75} />
					</button>
				{/snippet}
			</Menu>
		</span>
		<Button variant="primary" onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>

<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.name ?? ''}”?"
	description="Deleting this tag will remove it from all tasks."
	onconfirm={(tag) => store.remove(workspaceId, tag.id)}
/>
