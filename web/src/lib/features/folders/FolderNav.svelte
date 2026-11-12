<!-- A page's list of feeds, after the rows the page puts before it. They reorder by dragging, or with
     Alt+↑/↓. -->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import { navAction, navCount, navItem, navLink } from './nav';

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
		/** The nav's accessible name. */
		label?: string;
		/** The button that adds one. */
		newLabel?: string;
		/** How many of a folder's items are unread, counted after its title; none at 0. */
		unread?: (folder: Folder) => number;
		onnew: () => void;
		/** Every folder's id, in the order the user put them. */
		onreorder: (folderIds: number[]) => void;
	}

	let {
		folders,
		current,
		href,
		before,
		label = 'Folders',
		newLabel = 'New folder…',
		unread,
		onnew,
		onreorder
	}: Props = $props();

	const order = new Reorderable(
		() => folders,
		(folder) => folder.id,
		(ids) => onreorder(ids)
	);
</script>

<nav
	class="flex flex-col max-md:flex-row max-md:flex-wrap max-md:items-center max-md:gap-1"
	aria-label={label}
>
	<!-- Left out while empty (a page with no saved views yet), so its gap goes with it. -->
	{#if order.items.length > 0 || before}
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
						onkeydown={(e) => order.key(e, index)}
						>{folder.title}{#if unread?.(folder)}<span class="sr-only"
								>, {unread(folder)} unread</span
							>{/if}</a
					>
					{#if unread?.(folder)}
						<span class={navCount} aria-hidden="true">{unread(folder)}</span>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
	<div class="flex flex-col max-md:flex-row">
		<button class={navAction} type="button" onclick={onnew}>
			<!-- No glyph: the column has no icons, so the label keeps the rows' left edge; its grey says it's an action. -->
			{newLabel}
		</button>
	</div>
</nav>
