<!--
	The Documents page's folder switcher, under its heading: All, the Inbox, then every folder as
	a tree (a nested one a step in, folding away under its chevron, a table wearing a table's
	icon), and "New folder…". Typing in the filter lists the folders whose titles match, each
	with the folders holding it; ↓/↑ move through them and Enter opens the highlighted one. Each folder pins to the sidebar, makes a folder inside it,
	renames or deletes from its tools. Drag a folder onto another to nest it, or onto a folder's
	top or bottom edge to put it beside that one; Alt+↑/↓ moves it among its siblings.
-->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FileText from '@lucide/svelte/icons/file-text';
	import Folder from '@lucide/svelte/icons/folder';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import Inbox from '@lucide/svelte/icons/inbox';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Pin from '@lucide/svelte/icons/pin';
	import PinOff from '@lucide/svelte/icons/pin-off';
	import Table2 from '@lucide/svelte/icons/table-2';
	import Trash from '@lucide/svelte/icons/trash';
	import { SvelteSet } from 'svelte/reactivity';
	import {
		scrollIntoViewWhen,
		stepHighlight,
		switcherAction,
		switcherLink,
		switcherRow,
		switcherRule,
		switcherTool,
		switcherTools
	} from '$lib/features/folders/nav';
	import type { DocumentFolder } from '$lib/protocol/types';
	import { DocumentFolderDrag } from '$lib/shell/documentFolderDrag.svelte';
	import { folderDialogs } from '$lib/shell/folderDialogs.svelte';
	import SearchField from '$lib/ui/SearchField.svelte';
	import {
		type DocumentView,
		folderPath,
		folderTree,
		visibleNodes,
		type FolderNode
	} from './folderTree';

	interface Props {
		folders: DocumentFolder[];
		/** What the page shows. */
		current: DocumentView;
		href: (view: DocumentView) => string;
		/** Called as a row is picked, so the switcher closes. */
		onpick: () => void;
		/** Moves a folder into `parent_id` (the top level when null), before `before_id` or last. */
		onmove: (folder: DocumentFolder, parent_id: number | null, before_id: number | null) => void;
		onpin: (folder: DocumentFolder, pinned: boolean) => void;
	}

	let { folders, current, href, onpick, onmove, onpin }: Props = $props();

	let query = $state('');
	let active = $state(0);
	let root = $state<HTMLElement>();
	const collapsed = new SvelteSet<number>();
	const live = $derived(folders.filter((f) => !f.deleted));
	const rows = $derived(visibleNodes(folderTree(live), collapsed));
	const hasChildren = $derived(
		new Set(live.flatMap((f) => (f.parent_id === null ? [] : [f.parent_id])))
	);
	const matches = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (q === '') return null;
		return live
			.filter((f) => f.title.toLowerCase().includes(q))
			.map((folder) => ({
				folder,
				within: folderPath(live, folder.id)
					.slice(0, -1)
					.map((f) => f.title)
					.join(' › ')
			}));
	});

	// A new search starts on its first result.
	$effect.pre(() => {
		void query;
		active = 0;
	});

	function searchKey(e: KeyboardEvent) {
		const next = stepHighlight(e, active, matches?.length ?? 0);
		if (next !== null) active = next;
		else if (e.key === 'Enter') {
			// Enter's default action would land on the trigger the close hands focus back
			// to, and press it: the switcher would open again.
			e.preventDefault();
			root?.querySelector<HTMLAnchorElement>('[data-active=true] a')?.click();
		}
	}

	// The folder shown stays in sight: its parents open.
	$effect(() => {
		if (typeof current !== 'number') return;
		for (const f of folderPath(live, current)) if (f.id !== current) collapsed.delete(f.id);
	});

	function toggle(id: number) {
		if (collapsed.has(id)) collapsed.delete(id);
		else collapsed.add(id);
	}

	const drag = new DocumentFolderDrag(
		() => live,
		(folder, parent_id, before_id) => onmove(folder, parent_id, before_id),
		// Documents aren't dragged here; the page's list closes the switcher as a drag starts.
		() => {}
	);

	function newFolder(parent: DocumentFolder | null) {
		onpick();
		folderDialogs.request({ kind: 'documents', mode: 'new', parent });
	}
</script>

{#snippet fixedRow(view: 'all' | 'inbox', label: string)}
	{@const Icon = view === 'all' ? FileText : Inbox}
	<div class={switcherRow}>
		<a
			class={switcherLink}
			href={href(view)}
			aria-current={current === view ? 'page' : undefined}
			onclick={onpick}
		>
			<Icon size={14} strokeWidth={1.75} aria-hidden="true" />
			<span class="truncate">{label}</span>
		</a>
	</div>
{/snippet}

{#snippet tools(folder: DocumentFolder)}
	<span class={switcherTools}>
		<button
			class={switcherTool}
			type="button"
			aria-label="{folder.pinned ? 'Unpin' : 'Pin'} {folder.title}"
			onclick={() => onpin(folder, !folder.pinned)}
		>
			{#if folder.pinned}<PinOff size={13} strokeWidth={1.75} aria-hidden="true" />{:else}<Pin
					size={13}
					strokeWidth={1.75}
					aria-hidden="true"
				/>{/if}
		</button>
		<button
			class={switcherTool}
			type="button"
			aria-label="New folder in {folder.title}"
			onclick={() => newFolder(folder)}
		>
			<FolderPlus size={13} strokeWidth={1.75} aria-hidden="true" />
		</button>
		<button
			class={switcherTool}
			type="button"
			aria-label="Rename {folder.title}"
			onclick={() => {
				onpick();
				folderDialogs.request({ kind: 'documents', mode: 'rename', folder });
			}}
		>
			<Pencil size={13} strokeWidth={1.75} aria-hidden="true" />
		</button>
		<button
			class={switcherTool}
			type="button"
			aria-label="Delete {folder.title}"
			onclick={() => {
				onpick();
				folderDialogs.request({ kind: 'documents', mode: 'delete', folder });
			}}
		>
			<Trash size={13} strokeWidth={1.75} aria-hidden="true" />
		</button>
	</span>
{/snippet}

{#snippet folderIcon(folder: DocumentFolder)}
	{#if folder.representation === 'table'}
		<span class="sr-only">Table: </span>
		<Table2 size={14} strokeWidth={1.75} aria-hidden="true" />
	{:else}
		<Folder size={14} strokeWidth={1.75} aria-hidden="true" />
	{/if}
{/snippet}

{#snippet treeRow({ folder, depth }: FolderNode)}
	{@const drop = drag.drop?.target === folder.id ? drag.drop.where : null}
	<div
		class={[
			switcherRow,
			drag.dragged?.id === folder.id && 'opacity-50',
			drop === 'into' && 'bg-bg-hover ring-1 ring-accent ring-inset',
			drop === 'before' &&
				'before:absolute before:inset-x-1 before:-top-px before:h-0.5 before:rounded-full before:bg-accent',
			drop === 'after' &&
				'after:absolute after:inset-x-1 after:-bottom-px after:h-0.5 after:rounded-full after:bg-accent'
		]}
		style:padding-left="{depth * 14}px"
		role="presentation"
		draggable="true"
		ondragstart={(e) => drag.start(e, folder)}
		ondragend={() => drag.end()}
		ondragover={(e) => drag.over(e, folder)}
		ondragleave={(e) => drag.leave(e)}
		ondrop={(e) => drag.dropOn(e, folder)}
	>
		{#if hasChildren.has(folder.id)}
			<button
				class="-mr-1 ml-0.5 inline-flex size-5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:text-fg"
				type="button"
				aria-label="{collapsed.has(folder.id) ? 'Show' : 'Hide'} folders in {folder.title}"
				aria-expanded={!collapsed.has(folder.id)}
				onclick={() => toggle(folder.id)}
			>
				<ChevronRight
					class={['transition-transform', !collapsed.has(folder.id) && 'rotate-90']}
					size={13}
					strokeWidth={1.75}
					aria-hidden="true"
				/>
			</button>
		{:else}
			<span class="ml-0.5 w-4 flex-none" aria-hidden="true"></span>
		{/if}
		<a
			class={switcherLink}
			href={href(folder.id)}
			draggable="false"
			aria-current={current === folder.id ? 'page' : undefined}
			aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
			onclick={onpick}
			onkeydown={(e) => drag.key(e, folder)}
		>
			{@render folderIcon(folder)}
			<span class="truncate">{folder.title}</span>
		</a>
		{@render tools(folder)}
	</div>
{/snippet}

<div class="contents" bind:this={root}>
	<div class="p-1 pb-1.5">
		<SearchField
			bind:value={query}
			placeholder="Filter folders"
			label="Filter folders by title"
			onkeydown={searchKey}
		/>
	</div>

	{#if matches === null}
		{@render fixedRow('all', 'All documents')}
		{@render fixedRow('inbox', 'Inbox')}
		{#if rows.length > 0}
			<div class={switcherRule}></div>
			{#each rows as node (node.folder.id)}
				{@render treeRow(node)}
			{/each}
		{/if}
		<div class={switcherRule}></div>
		<button class={switcherAction} type="button" onclick={() => newFolder(null)}>
			<FolderPlus size={14} strokeWidth={1.75} aria-hidden="true" />New folder…
		</button>
	{:else if matches.length === 0}
		<p class="px-2 py-1.5 text-fg-muted">No folders match “{query.trim()}”.</p>
	{:else}
		{#each matches as { folder, within }, i (folder.id)}
			<div
				class={[switcherRow, i === active && 'bg-bg-hover']}
				data-active={i === active}
				{@attach (n) => scrollIntoViewWhen(n, i === active)}
			>
				<a
					class={[switcherLink, 'py-1']}
					href={href(folder.id)}
					aria-current={current === folder.id ? 'page' : undefined}
					onclick={onpick}
				>
					{@render folderIcon(folder)}
					<span class="flex min-w-0 flex-col">
						<span class="truncate">{folder.title}</span>
						{#if within}<span class="truncate text-xs text-fg-subtle">{within}</span>{/if}
					</span>
				</a>
				{@render tools(folder)}
			</div>
		{/each}
	{/if}
</div>
