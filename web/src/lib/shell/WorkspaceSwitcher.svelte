<!--
	The sidebar's workspace picker: a popover to switch to or create a
	workspace, and beside it a menu for the current one (its Rewind page, what
	the sidebar shows of it, edit, delete). The rail and the phone top bar show the picker as only its badge:
	the workspace's emoji, or its initial when it has none.
-->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import Check from '@lucide/svelte/icons/check';
	import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import EmojiPicker from '$lib/ui/EmojiPicker.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Menu from '$lib/ui/Menu.svelte';
	import Popover from '$lib/ui/Popover.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Workspace } from '$lib/protocol/types';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import { routes, workspaceSwitchRoute } from './routes';
	import WorkspaceBadge from './WorkspaceBadge.svelte';

	/** Opens the dialog choosing which kinds of resource the sidebar shows. */
	let { onchoosekinds }: { onchoosekinds: () => void } = $props();

	let open = $state(false);
	let dialog = $state<'create' | 'edit' | 'delete' | null>(null);
	// Whether the dialog shows. `dialog` keeps its kind after closing, so the
	// fade-out doesn't switch layouts.
	let dialogOpen = $state(false);
	let target = $state<Workspace | null>(null);
	let title = $state('');
	let emoji = $state('');
	let fieldError = $state<string | null>(null);
	let emojiError = $state<string | null>(null);
	let busy = $state(false);

	const current = $derived(workspaces.current);

	const pick =
		'flex min-w-0 flex-1 cursor-pointer items-center justify-between gap-2 rounded-sm px-2 py-1 text-left hover:bg-bg-hover';

	function startDialog(kind: 'create' | 'edit' | 'delete', workspace: Workspace | null) {
		open = false;
		target = workspace;
		title = workspace?.title ?? '';
		emoji = workspace?.emoji ?? '';
		fieldError = null;
		emojiError = null;
		dialog = kind;
		dialogOpen = true;
	}

	function switchTo(id: number) {
		open = false;
		void goto(workspaceSwitchRoute(page.url.pathname, id));
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		fieldError = null;
		emojiError = null;
		try {
			if (dialog === 'create') {
				const created = await workspaces.create(title.trim(), emoji.trim() || null);
				dialogOpen = false;
				void goto(routes.workspace(created.id));
			} else if (dialog === 'edit' && target) {
				if (title.trim() !== target.title) await workspaces.rename(target.id, title.trim());
				if (emoji.trim() !== (target.emoji ?? ''))
					await workspaces.setEmoji(target.id, emoji.trim() || null);
				dialogOpen = false;
			} else if (dialog === 'delete' && target) {
				const deletingCurrent = target.id === workspaces.currentId;
				await workspaces.remove(target.id);
				dialogOpen = false;
				if (deletingCurrent) void goto(resolve('/'));
			}
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed' && e.field === 'emoji') emojiError = e.message;
			else if (e.code === 'validationFailed') fieldError = e.message;
			else toasts.error(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="flex flex-none items-center gap-px max-md:flex-col max-sm:flex-row">
	<Popover bind:open>
		{#snippet trigger(props)}
			<button
				class="flex h-8 min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-sm px-2 text-left text-md font-semibold text-fg hover:bg-bg-hover max-md:w-9 max-md:flex-none max-md:justify-center max-md:p-0"
				type="button"
				{...props}
				aria-label="Workspace: {current?.title ?? 'none'}"
			>
				<WorkspaceBadge workspace={current} />
				<span class="min-w-0 flex-1 truncate max-md:hidden">{current?.title ?? 'Workspaces'}</span>
				<ChevronsUpDown
					class="flex-none text-fg-subtle max-md:hidden"
					size={14}
					strokeWidth={1.75}
				/>
			</button>
		{/snippet}
		<ul>
			{#each workspaces.list.value as workspace (workspace.id)}
				<li>
					<button
						class={[pick, 'w-full text-fg']}
						type="button"
						onclick={() => switchTo(workspace.id)}
					>
						<span class="flex min-w-0 items-center gap-2">
							<WorkspaceBadge {workspace} />
							<span class="truncate">{workspace.title}</span>
						</span>
						{#if workspace.id === workspaces.currentId}<Check size={14} strokeWidth={1.75} />{/if}
					</button>
				</li>
			{/each}
		</ul>
		<div class="mt-1 border-t border-border pt-1">
			<button
				class={[pick, 'w-full text-fg-muted']}
				type="button"
				onclick={() => startDialog('create', null)}
			>
				New workspace…
			</button>
		</div>
	</Popover>
	{#if current}
		<span class="flex">
			<Menu
				items={[
					{ label: 'Rewind', onSelect: () => void goto(routes.rewind(current.id)) },
					{ label: 'Show in sidebar…', onSelect: onchoosekinds },
					{ label: 'Edit…', onSelect: () => startDialog('edit', current) },
					{ label: 'Delete…', danger: true, onSelect: () => startDialog('delete', current) }
				]}
			>
				{#snippet trigger(props)}
					<button
						class="inline-grid size-7 cursor-pointer place-items-center rounded-sm text-fg-subtle hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg max-md:h-7.5 max-md:w-9"
						type="button"
						aria-label="More for {current.title}"
						{...props}
					>
						<Ellipsis size={14} strokeWidth={1.75} />
					</button>
				{/snippet}
			</Menu>
		</span>
	{/if}
</div>

<Dialog
	bind:open={dialogOpen}
	title={dialog === 'create'
		? 'New workspace'
		: dialog === 'edit'
			? 'Edit workspace'
			: `Delete “${target?.title ?? ''}”?`}
	description={dialog === 'delete'
		? 'All journal entries, documents, tasks and resources in this workspace will also be deleted.'
		: undefined}
>
	<form id="workspace-form" class="flex flex-col gap-3" onsubmit={submit}>
		{#if dialog !== 'delete'}
			<Field label="Title" for="workspace-title" error={emojiError ?? fieldError}>
				<div class="flex gap-2">
					<EmojiPicker bind:value={emoji} />
					<TextInput id="workspace-title" bind:value={title} autocomplete="off" required />
				</div>
			</Field>
		{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (dialogOpen = false)}>Cancel</Button>
		{#if dialog === 'delete'}
			<Button variant="danger-quiet" type="submit" form="workspace-form" disabled={busy}>
				Delete
			</Button>
		{:else}
			<Button
				variant="primary"
				type="submit"
				form="workspace-form"
				disabled={busy || title.trim() === ''}
			>
				{dialog === 'create' ? 'Create' : 'Save'}
			</Button>
		{/if}
	{/snippet}
</Dialog>
