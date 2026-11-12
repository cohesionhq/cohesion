<!--
	The Tasks page's switcher, under its heading: All and the Inbox, the folders, Done and
	Cancelled, then the saved views, and "New folder…". Typing in the filter lists the folders
	and views whose names match. A folder is drawn in its color, and pins to the sidebar, takes a color, renames or deletes from its tools;
	a view renames or deletes. Folders and views reorder by dragging, or with Alt+↑/↓.
-->
<script lang="ts">
	import Ban from '@lucide/svelte/icons/ban';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Folder from '@lucide/svelte/icons/folder';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import Inbox from '@lucide/svelte/icons/inbox';
	import Layers from '@lucide/svelte/icons/layers';
	import ListFilter from '@lucide/svelte/icons/list-filter';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Pin from '@lucide/svelte/icons/pin';
	import PinOff from '@lucide/svelte/icons/pin-off';
	import Trash from '@lucide/svelte/icons/trash';
	import type { Component } from 'svelte';
	import {
		switcherAction,
		switcherHeading,
		switcherLink,
		switcherRow,
		switcherRule,
		switcherTool,
		switcherTools
	} from '$lib/features/folders/nav';
	import type { IntelligenceAgentColor, SavedTaskView, TaskFolder } from '$lib/protocol/types';
	import { folderDialogs } from '$lib/shell/folderDialogs.svelte';
	import { savedViewQuery, type TaskView } from '$lib/stores/tasks.svelte';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import ColorPicker from '$lib/ui/ColorPicker.svelte';
	import { agentText } from '$lib/ui/colors';
	import SearchField from '$lib/ui/SearchField.svelte';

	interface Props {
		folders: TaskFolder[];
		views: SavedTaskView[];
		/** The list shown, unless a saved view keeps what's shown. */
		current: TaskView;
		/** The saved view keeping exactly what's shown, if one does. */
		currentView: number | null;
		/** The Tasks page's address, before its query. */
		base: string;
		/** Called as a row is picked, so the switcher closes. */
		onpick: () => void;
		/** Called as a saved view is opened, so the page takes its order. */
		onpickview: (view: SavedTaskView) => void;
		onpin: (folder: TaskFolder, pinned: boolean) => void;
		oncolor: (folder: TaskFolder, color: IntelligenceAgentColor | null) => void;
		/** Every folder's id, in the order the user put them. */
		onreorderfolders: (folderIds: number[]) => void;
		/** Every saved view's id, in the order the user put them. */
		onreorderviews: (viewIds: number[]) => void;
	}

	let {
		folders,
		views,
		current,
		currentView,
		base,
		onpick,
		onpickview,
		onpin,
		oncolor,
		onreorderfolders,
		onreorderviews
	}: Props = $props();

	let query = $state('');
	const q = $derived(query.trim().toLowerCase());
	const live = $derived(folders.filter((f) => !f.deleted));

	const folderOrder = new Reorderable(
		() => live,
		(folder) => folder.id,
		(ids) => onreorderfolders(ids)
	);
	const viewOrder = new Reorderable(
		() => views,
		(view) => view.id,
		(ids) => onreorderviews(ids)
	);

	const shownFolders = $derived(
		q === '' ? folderOrder.items : live.filter((f) => f.title.toLowerCase().includes(q))
	);
	const shownViews = $derived(
		q === '' ? viewOrder.items : views.filter((v) => v.name.toLowerCase().includes(q))
	);

	const isCurrent = (list: TaskView) => currentView === null && current === list;
	const href = (list: TaskView) => `${base}?view=${list}`;

	function ask(request: Parameters<typeof folderDialogs.request>[0]) {
		onpick();
		folderDialogs.request(request);
	}
</script>

{#snippet listRow(list: TaskView, label: string, Icon: Component)}
	<div class={switcherRow}>
		<a
			class={switcherLink}
			href={href(list)}
			aria-current={isCurrent(list) ? 'page' : undefined}
			onclick={onpick}
		>
			<Icon size={14} strokeWidth={1.75} aria-hidden="true" />
			<span class="truncate">{label}</span>
		</a>
	</div>
{/snippet}

{#snippet folderRow(folder: TaskFolder, index: number)}
	<div
		class={[switcherRow, folderOrder.dragged === folder.id && 'opacity-50']}
		role="presentation"
		draggable={q === '' ? 'true' : undefined}
		ondragstart={(e) => folderOrder.start(e, folder, folder.title)}
		{...folderOrder.target(index)}
		ondragend={() => folderOrder.end()}
	>
		<a
			class={switcherLink}
			href={href(folder.id)}
			draggable="false"
			aria-current={isCurrent(folder.id) ? 'page' : undefined}
			aria-keyshortcuts={q === '' ? 'Alt+ArrowUp Alt+ArrowDown' : undefined}
			onclick={onpick}
			onkeydown={(e) => q === '' && folderOrder.key(e, index)}
		>
			{#if folder.color}
				<!-- The link quiets every icon, so a colored one takes its color from a wrapper. -->
				<span class={['inline-flex flex-none', agentText[folder.color]]}>
					<Folder class="text-current!" size={14} strokeWidth={1.75} aria-hidden="true" />
				</span>
			{:else}
				<Folder size={14} strokeWidth={1.75} aria-hidden="true" />
			{/if}
			<span class="truncate">{folder.title}</span>
		</a>
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
			<ColorPicker
				color={folder.color}
				label="Color of {folder.title}"
				onchange={(color) => oncolor(folder, color)}
				compact
			/>
			<button
				class={switcherTool}
				type="button"
				aria-label="Rename {folder.title}"
				onclick={() => ask({ kind: 'tasks', mode: 'rename', folder })}
			>
				<Pencil size={13} strokeWidth={1.75} aria-hidden="true" />
			</button>
			<button
				class={switcherTool}
				type="button"
				aria-label="Delete {folder.title}"
				onclick={() => ask({ kind: 'tasks', mode: 'delete', folder })}
			>
				<Trash size={13} strokeWidth={1.75} aria-hidden="true" />
			</button>
		</span>
	</div>
{/snippet}

{#snippet viewRow(view: SavedTaskView, index: number)}
	<div
		class={[switcherRow, viewOrder.dragged === view.id && 'opacity-50']}
		role="presentation"
		draggable={q === '' ? 'true' : undefined}
		ondragstart={(e) => viewOrder.start(e, view, view.name)}
		{...viewOrder.target(index)}
		ondragend={() => viewOrder.end()}
	>
		<a
			class={switcherLink}
			href="{base}{savedViewQuery(view)}"
			draggable="false"
			aria-current={currentView === view.id ? 'page' : undefined}
			aria-keyshortcuts={q === '' ? 'Alt+ArrowUp Alt+ArrowDown' : undefined}
			onclick={() => {
				onpickview(view);
				onpick();
			}}
			onkeydown={(e) => q === '' && viewOrder.key(e, index)}
		>
			<ListFilter size={14} strokeWidth={1.75} aria-hidden="true" />
			<span class="truncate">{view.name}</span>
		</a>
		<span class={switcherTools}>
			<button
				class={switcherTool}
				type="button"
				aria-label="Rename {view.name}"
				onclick={() => ask({ kind: 'views', mode: 'rename', view })}
			>
				<Pencil size={13} strokeWidth={1.75} aria-hidden="true" />
			</button>
			<button
				class={switcherTool}
				type="button"
				aria-label="Delete {view.name}"
				onclick={() => ask({ kind: 'views', mode: 'delete', view })}
			>
				<Trash size={13} strokeWidth={1.75} aria-hidden="true" />
			</button>
		</span>
	</div>
{/snippet}

<div class="p-1 pb-1.5">
	<SearchField bind:value={query} placeholder="Filter folders" label="Filter folders and views" />
</div>

{#if q === ''}
	{@render listRow('all', 'All', Layers)}
	{@render listRow('inbox', 'Inbox', Inbox)}
	{#if shownFolders.length > 0}
		<div class={switcherRule}></div>
		{#each shownFolders as folder, index (folder.id)}
			{@render folderRow(folder, index)}
		{/each}
	{/if}
	<div class={switcherRule}></div>
	{@render listRow('done', 'Done', CircleCheck)}
	{@render listRow('cancelled', 'Cancelled', Ban)}
	{#if shownViews.length > 0}
		<div class={switcherHeading}>Views</div>
		{#each shownViews as view, index (view.id)}
			{@render viewRow(view, index)}
		{/each}
	{/if}
	<div class={switcherRule}></div>
	<button class={switcherAction} type="button" onclick={() => ask({ kind: 'tasks', mode: 'new' })}>
		<FolderPlus size={14} strokeWidth={1.75} aria-hidden="true" />New folder…
	</button>
{:else if shownFolders.length === 0 && shownViews.length === 0}
	<p class="px-2 py-1.5 text-fg-muted">Nothing matches “{query.trim()}”.</p>
{:else}
	{#each shownFolders as folder, index (folder.id)}
		{@render folderRow(folder, index)}
	{/each}
	{#if shownViews.length > 0}
		<div class={switcherHeading}>Views</div>
		{#each shownViews as view, index (view.id)}
			{@render viewRow(view, index)}
		{/each}
	{/if}
{/if}
