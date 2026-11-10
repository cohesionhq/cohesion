<!-- The Documents page's folders as a tree, under Inbox and All; a folder that is a table wears a table's icon.
     Drag a document (or a table's row) onto a folder or the Inbox to file it there; drag a folder onto another to nest it, or onto a folder's top or bottom edge to put it
     beside that one. Alt+↑/↓ moves a folder among its siblings, as task folders reorder. (Alt+←/→ are the app's
     back and forward.) -->
<script lang="ts" module>
	/** The drag data a document row sets, holding its id. */
	export const DOCUMENT_DRAG = 'application/x-cohesion-document';
	const FOLDER_DRAG = 'application/x-cohesion-document-folder';

	import type { DocumentView as View } from './folderTree';

	/** What the list shows: the Inbox, every document, or one folder by id. */
	export type DocumentView = View;
</script>

<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Table2 from '@lucide/svelte/icons/table-2';
	import Trash from '@lucide/svelte/icons/trash';
	import { SvelteSet } from 'svelte/reactivity';
	import { navAction, navHeading, navItem, navLink, navTool } from '$lib/features/folders/nav';
	import type { DocumentFolder } from '$lib/protocol/types';
	import { folderPath, folderTree, siblings, subtreeIds, visibleNodes } from './folderTree';

	interface Props {
		folders: DocumentFolder[];
		current: DocumentView;
		href: (view: DocumentView) => string;
		/** Creates a folder inside `parent`, or at the top level when it's null. */
		onnew: (parent: DocumentFolder | null) => void;
		onrename: (folder: DocumentFolder) => void;
		ondelete: (folder: DocumentFolder) => void;
		/** Moves a folder into `parent_id` (the top level when null), before `before_id` or last. */
		onmove: (folder: DocumentFolder, parent_id: number | null, before_id: number | null) => void;
		/** Files a dragged document in `folder_id`, or the Inbox when it's null. */
		onfile: (document_id: number, folder_id: number | null) => void;
	}

	let { folders, current, href, onnew, onrename, ondelete, onmove, onfile }: Props = $props();

	const collapsed = new SvelteSet<number>();
	const tree = $derived(folderTree(folders));
	const rows = $derived(visibleNodes(tree, collapsed));

	// The folder shown stays in sight: its parents open as it's picked.
	$effect(() => {
		if (typeof current !== 'number') return;
		for (const f of folderPath(folders, current)) if (f.id !== current) collapsed.delete(f.id);
	});

	type Where = 'before' | 'after' | 'into';
	let dragged = $state<DocumentFolder | null>(null);
	let drop = $state<{ target: number | 'inbox'; where: Where } | null>(null);

	const carries = (event: DragEvent, type: string) =>
		event.dataTransfer?.types.includes(type) ?? false;

	/** Where a drag over `folder`'s row would land, or null where it can't. */
	function landing(event: DragEvent, folder: DocumentFolder): Where | null {
		if (carries(event, DOCUMENT_DRAG)) return 'into';
		if (!dragged || !carries(event, FOLDER_DRAG)) return null;
		if (subtreeIds(folders, dragged.id).includes(folder.id)) return null;
		const { top, height } = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const y = (event.clientY - top) / height;
		return y < 0.25 ? 'before' : y > 0.75 ? 'after' : 'into';
	}

	function over(event: DragEvent, folder: DocumentFolder) {
		const where = landing(event, folder);
		if (where === null) return;
		event.preventDefault();
		event.dataTransfer!.dropEffect = 'move';
		drop = { target: folder.id, where };
	}

	function dropOn(event: DragEvent, folder: DocumentFolder) {
		const where = landing(event, folder);
		drop = null;
		if (where === null) return;
		event.preventDefault();
		const documentId = event.dataTransfer!.getData(DOCUMENT_DRAG);
		if (documentId) return onfile(Number(documentId), folder.id);
		const moving = dragged!;
		if (where === 'into') {
			if (moving.parent_id !== folder.id) onmove(moving, folder.id, null);
			return;
		}
		const row = siblings(folders, folder).filter((f) => f.id !== moving.id);
		const after = row[row.findIndex((f) => f.id === folder.id) + 1];
		onmove(moving, folder.parent_id, where === 'before' ? folder.id : (after?.id ?? null));
	}

	// The Inbox takes a document, and a folder there goes to the top level.
	function overInbox(event: DragEvent) {
		if (!carries(event, DOCUMENT_DRAG) && !(dragged && carries(event, FOLDER_DRAG))) return;
		event.preventDefault();
		drop = { target: 'inbox', where: 'into' };
	}

	function dropOnInbox(event: DragEvent) {
		drop = null;
		event.preventDefault();
		const documentId = event.dataTransfer?.getData(DOCUMENT_DRAG);
		if (documentId) onfile(Number(documentId), null);
		else if (dragged && dragged.parent_id !== null) onmove(dragged, null, null);
	}

	function start(event: DragEvent, folder: DocumentFolder) {
		dragged = folder;
		event.dataTransfer!.effectAllowed = 'move';
		event.dataTransfer!.setData(FOLDER_DRAG, String(folder.id));
		// Firefox only starts a drag that carries data; a drop elsewhere gets the title.
		event.dataTransfer!.setData('text/plain', folder.title);
	}

	function end() {
		dragged = null;
		drop = null;
	}

	function leave(event: DragEvent) {
		const to = event.relatedTarget as Node | null;
		if (!to || !(event.currentTarget as HTMLElement).contains(to)) drop = null;
	}

	function key(event: KeyboardEvent, folder: DocumentFolder) {
		if (!event.altKey) return;
		const row = siblings(folders, folder);
		const index = row.findIndex((f) => f.id === folder.id);
		if (event.key === 'ArrowUp' && index > 0) onmove(folder, folder.parent_id, row[index - 1].id);
		else if (event.key === 'ArrowDown' && index < row.length - 1)
			onmove(folder, folder.parent_id, row[index + 2]?.id ?? null);
		else return;
		event.preventDefault();
	}

	const toggle = (id: number) => (collapsed.has(id) ? collapsed.delete(id) : collapsed.add(id));
</script>

<nav class="flex flex-col" aria-label="Folders">
	<ul>
		<li
			class={[navItem, drop?.target === 'inbox' && 'bg-bg-hover ring-1 ring-accent ring-inset']}
			ondragover={overInbox}
			ondragleave={leave}
			ondrop={dropOnInbox}
		>
			<a
				class={navLink}
				href={href('inbox')}
				aria-current={current === 'inbox' ? 'page' : undefined}>Inbox</a
			>
		</li>
		<li class={navItem}>
			<a class={navLink} href={href('all')} aria-current={current === 'all' ? 'page' : undefined}
				>All</a
			>
		</li>
		<li class={navHeading}>Folders</li>
	</ul>
	{#if rows.length > 0}
		<ul>
			{#each rows as { folder, depth, children } (folder.id)}
				{@const open = !collapsed.has(folder.id)}
				{@const target = drop?.target === folder.id ? drop.where : null}
				<li
					class={[
						'group/folder relative',
						navItem,
						dragged?.id === folder.id && 'opacity-50',
						target === 'into' && 'bg-bg-hover ring-1 ring-accent ring-inset',
						target === 'before' &&
							'before:absolute before:inset-x-1 before:-top-px before:h-0.5 before:rounded-full before:bg-accent',
						target === 'after' &&
							'after:absolute after:inset-x-1 after:-bottom-px after:h-0.5 after:rounded-full after:bg-accent'
					]}
					style:padding-left="{depth * 0.875}rem"
					draggable="true"
					ondragstart={(e) => start(e, folder)}
					ondragend={end}
					ondragover={(e) => over(e, folder)}
					ondragleave={leave}
					ondrop={(e) => dropOn(e, folder)}
				>
					{#if children.length > 0}
						<button
							class="inline-flex size-5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:text-fg"
							type="button"
							aria-label="{open ? 'Collapse' : 'Expand'} {folder.title}"
							aria-expanded={open}
							onclick={() => toggle(folder.id)}
						>
							<ChevronRight
								class={['transition-transform', open && 'rotate-90']}
								size={13}
								strokeWidth={1.75}
							/>
						</button>
					{:else}
						<span class="size-5 flex-none" aria-hidden="true"></span>
					{/if}
					{#if folder.is_table}
						<Table2
							class="ml-1 flex-none text-fg-subtle"
							size={13}
							strokeWidth={1.75}
							aria-hidden="true"
						/>
					{/if}
					<a
						class={[navLink, 'pl-1']}
						href={href(folder.id)}
						draggable="false"
						aria-current={current === folder.id ? 'page' : undefined}
						aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
						onkeydown={(e) => key(e, folder)}
						>{folder.title}{#if folder.is_table}<span class="sr-only">, table</span>{/if}</a
					>
					<span
						class={[
							'flex opacity-0 transition-opacity group-focus-within/folder:opacity-100 group-hover/folder:opacity-100 touch:opacity-100',
							current !== folder.id && 'max-md:hidden'
						]}
					>
						<button
							class={navTool}
							type="button"
							aria-label="New folder in {folder.title}"
							onclick={() => onnew(folder)}
						>
							<FolderPlus size={13} strokeWidth={1.75} />
						</button>
						<button
							class={navTool}
							type="button"
							aria-label="Rename {folder.title}"
							onclick={() => onrename(folder)}
						>
							<Pencil size={13} strokeWidth={1.75} />
						</button>
						<button
							class={navTool}
							type="button"
							aria-label="Delete {folder.title}"
							onclick={() => ondelete(folder)}
						>
							<Trash size={13} strokeWidth={1.75} />
						</button>
					</span>
				</li>
			{/each}
		</ul>
	{/if}
	<button class={navAction} type="button" onclick={() => onnew(null)}>New folder…</button>
</nav>
