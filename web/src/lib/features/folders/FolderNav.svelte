<!-- A page's folders, between the views the page puts before and after them (an inbox, All). Folders
     reorder by dragging, or with Alt+↑/↓. The Articles page lists its feeds with it too. -->
<script lang="ts">
	import Pencil from '@lucide/svelte/icons/pencil';
	import Trash from '@lucide/svelte/icons/trash';
	import type { Snippet } from 'svelte';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import { navAction, navCount, navItem, navLink, navTool } from './nav';

	interface Folder {
		id: number;
		title: string;
	}

	interface Props {
		folders: Folder[];
		/** The folder shown, if the view is one. */
		current: number | null;
		href: (folderId: number) => string;
		/** Rows before the folders, each an `<li>` styled with `navItem` and `navLink`. */
		before?: Snippet;
		/** Rows after the folders. */
		after?: Snippet;
		/** Buttons under the list, after "New folder…"; styled with `navAction`. */
		actions?: Snippet;
		/** The nav's accessible name. */
		label?: string;
		/** The button that adds one. */
		newLabel?: string;
		/** How many of a folder's items are unread, counted after its title; none at 0. */
		unread?: (folder: Folder) => number;
		onnew: () => void;
		/** Called as a folder's link is followed, before the page changes. */
		onpick?: (folder: Folder) => void;
		/** More buttons for a folder, before rename and delete; styled with `navTool`. */
		tools?: Snippet<[Folder]>;
		/** Without these, a folder has no rename and delete buttons. */
		onrename?: (folder: Folder) => void;
		ondelete?: (folder: Folder) => void;
		/** Every folder's id, in the order the user put them. */
		onreorder: (folderIds: number[]) => void;
	}

	let {
		folders,
		current,
		href,
		before,
		after,
		actions,
		label = 'Folders',
		newLabel = 'New folder…',
		unread,
		onpick,
		tools,
		onnew,
		onrename,
		ondelete,
		onreorder
	}: Props = $props();

	const order = new Reorderable(
		() => folders,
		(folder) => folder.id,
		(ids) => onreorder(ids)
	);
</script>

<nav
	class="flex flex-col gap-3 max-md:flex-row max-md:flex-wrap max-md:items-center max-md:gap-1"
	aria-label={label}
>
	<ul class="max-md:flex max-md:flex-wrap max-md:gap-1">
		{@render before?.()}
		{#each order.items as folder, index (folder.id)}
			<li
				class={['group/folder', navItem, order.dragged === folder.id && 'opacity-50']}
				draggable="true"
				ondragstart={(e) => order.start(e, folder, folder.title)}
				{...order.target(index)}
				ondragend={() => order.end()}
			>
				<a
					class={navLink}
					href={href(folder.id)}
					draggable="false"
					aria-current={current === folder.id ? 'page' : undefined}
					aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
					onclick={() => onpick?.(folder)}
					onkeydown={(e) => order.key(e, index)}
					>{folder.title}{#if unread?.(folder)}<span class="sr-only">, {unread(folder)} unread</span
						>{/if}</a
				>
				{#if unread?.(folder)}
					<span class={navCount} aria-hidden="true">{unread(folder)}</span>
				{/if}
				{#if onrename && ondelete}<span
						class={[
							'flex opacity-0 transition-opacity group-focus-within/folder:opacity-100 group-hover/folder:opacity-100 touch:opacity-100 max-md:opacity-100',
							current !== folder.id && 'max-md:hidden'
						]}
					>
						{@render tools?.(folder)}
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
					</span>{/if}
			</li>
		{/each}
		{@render after?.()}
	</ul>
	<div class="flex flex-col max-md:flex-row">
		<button class={navAction} type="button" onclick={onnew}>{newLabel}</button>
		{@render actions?.()}
	</div>
</nav>
