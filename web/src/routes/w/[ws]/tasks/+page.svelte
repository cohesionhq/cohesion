<script lang="ts">
	import { tick, untrack } from 'svelte';
	import FolderNav from '$lib/features/folders/FolderNav.svelte';
	import { navHeading, navItem, navLink, navPane } from '$lib/features/folders/nav';
	import AddTaskRow from '$lib/features/tasks/AddTaskRow.svelte';
	import SavedViewsNav from '$lib/features/tasks/SavedViewsNav.svelte';
	import SortMenu from '$lib/features/tasks/SortMenu.svelte';
	import TaskDetails from '$lib/features/tasks/TaskDetails.svelte';
	import TaskFilters from '$lib/features/tasks/TaskFilters.svelte';
	import TagsDialog from '$lib/features/tags/TagsDialog.svelte';
	import TaskRow from '$lib/features/tasks/TaskRow.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type {
		SavedTaskView,
		Task,
		TaskFolder,
		TaskPriority,
		TaskSize,
		TaskStatus
	} from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { routes } from '$lib/shell/routes';
	import { isTyping } from '$lib/shell/shortcuts.svelte';
	import {
		parsePriorities,
		parseSizes,
		parseTags,
		parseView,
		prioritiesQuery,
		rememberOrder,
		rememberShowIds,
		rememberedShowIds,
		rememberView,
		rememberedOrder,
		rememberedView,
		savedAs,
		sizesQuery,
		spansFolders,
		tagsQuery,
		viewFolder,
		viewOf,
		viewStatus,
		type TaskOrder,
		type TaskView
	} from '$lib/stores/tasks.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import Field from '$lib/ui/Field.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import Pager, { rememberedPageSize } from '$lib/ui/Pager.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { tasks, taskTags, contacts } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const viewParam = $derived(page.url.searchParams.get('view'));
	// Without `?view=` (the sidebar's link, say), the view last shown here.
	const view = $derived.by((): TaskView => {
		if (viewParam !== null) return parseView(viewParam);
		const remembered = rememberedView(ws);
		// A remembered folder may have been deleted since.
		const gone =
			typeof remembered === 'number' &&
			tasks.folders.key === ws &&
			tasks.folders.loaded &&
			!tasks.folders.value.some((f) => f.id === remembered);
		return gone ? 'inbox' : remembered;
	});

	$effect(() => rememberView(ws, view));

	let order = $state<TaskOrder>(rememberedOrder());

	let showIds = $state(rememberedShowIds());

	function setShowIds(show: boolean) {
		showIds = show;
		rememberShowIds(show);
	}

	function setOrder(next: TaskOrder) {
		order = next;
		rememberOrder(next);
		showPage(0);
	}

	// `?page=` counts from 1; a view opens on its first.
	let limit = $state(rememberedPageSize());
	const offset = $derived(
		Math.max(0, Number(page.url.searchParams.get('page') ?? 1) - 1 || 0) * limit
	);

	function showPage(next: number) {
		const url = new URL(page.url);
		if (next === 0) url.searchParams.delete('page');
		else url.searchParams.set('page', String(next / limit + 1));
		void page.goto(`${url.pathname}${url.search}`, { keepFocus: true });
	}

	function showSize(size: number) {
		limit = size;
		showPage(0);
	}

	// `?tag=` narrows the view to the tasks with every tag given, `?without=` to those with none.
	const tagsParam = $derived(parseTags(page.url.searchParams));
	// Of those, the tags that still exist.
	const tags = $derived.by(() => {
		const known = (id: number) => taskTags.list.value.some((t) => t.id === id);
		return { with: tagsParam.with.filter(known), without: tagsParam.without.filter(known) };
	});

	function tagState(id: number): 'with' | 'without' | null {
		if (tags.with.includes(id)) return 'with';
		if (tags.without.includes(id)) return 'without';
		return null;
	}

	// `?priority=` keeps the tasks with any of the priorities given.
	const priorityParam = $derived(parsePriorities(page.url.searchParams));
	// `?size=` keeps the tasks with any of the sizes given.
	const sizeParam = $derived(parseSizes(page.url.searchParams));

	$effect(() => {
		const [w, v, o, t, p, s, off, l] = [
			ws,
			view,
			order,
			tagsParam,
			priorityParam,
			sizeParam,
			offset,
			limit
		];
		untrack(() => {
			void tasks.open(w, v, o, t, p, s, off, l);
			void taskTags.load(w);
			void contacts.load(w);
		});
	});

	// A page past the end (its last tasks just went) shows the last one there is.
	$effect(() => {
		const { tasks: listed, total } = tasks.list.value;
		if (tasks.list.loaded && listed.length === 0 && total > 0 && offset > 0)
			untrack(() => showPage(Math.floor((total - 1) / limit) * limit));
	});

	function viewHref(v: TaskView): string {
		return `${routes.tasks(ws)}?view=${v}`;
	}

	// Picking a tag shows only what has it, then hides what has it, then neither.
	function tagHref(id: number): string {
		const others = (ids: number[]) => ids.filter((other) => other !== id);
		const state = tagState(id);
		const next = {
			with: state === null ? [...tags.with, id] : others(tags.with),
			without: state === 'with' ? [...tags.without, id] : others(tags.without)
		};
		return `${viewHref(view)}${tagsQuery(next)}${prioritiesQuery(priorityParam)}${sizesQuery(sizeParam)}`;
	}

	// Picking a priority adds it to those kept, or takes it out again.
	function priorityHref(priority: TaskPriority): string {
		const next = priorityParam.includes(priority)
			? priorityParam.filter((p) => p !== priority)
			: [...priorityParam, priority];
		return `${viewHref(view)}${tagsQuery(tags)}${prioritiesQuery(next)}${sizesQuery(sizeParam)}`;
	}

	// Picking a size adds it to those kept, or takes it out again.
	function sizeHref(size: TaskSize): string {
		const next = sizeParam.includes(size)
			? sizeParam.filter((s) => s !== size)
			: [...sizeParam, size];
		return `${viewHref(view)}${tagsQuery(tags)}${prioritiesQuery(priorityParam)}${sizesQuery(next)}`;
	}

	let managingTags = $state(false);

	// ---- Saved views --------------------------------------------------------

	const filters = $derived({ view, tags, priorities: priorityParam, sizes: sizeParam, order });
	// The saved view keeping exactly what's shown, which the nav marks instead of its list.
	const saved = $derived(
		tasks.views.key === ws ? (tasks.views.value.find((v) => savedAs(v, filters)) ?? null) : null
	);
	const shown = $derived(saved === null ? view : null);

	function savedHref(v: SavedTaskView): string {
		return `${viewHref(v.scope)}${tagsQuery(v.tags)}${prioritiesQuery(v.priorities)}${sizesQuery(v.sizes)}`;
	}

	// The link carries the list, tags, priorities and sizes; the order is the page's own.
	function pickSaved(v: SavedTaskView) {
		order = { sort: v.sort, descending: v.descending };
		rememberOrder(order);
	}

	async function setTagged(task: Task, tag_id: number, tagged: boolean) {
		tasks.takeTagged(await taskTags.setTagged(ws, tag_id, task.id, tagged));
	}

	const folder = $derived(
		typeof view === 'number' ? (tasks.folders.value.find((f) => f.id === view) ?? null) : null
	);
	const folderGone = $derived(typeof view === 'number' && tasks.folders.loaded && folder === null);
	const viewName = $derived(
		saved !== null
			? saved.name
			: view === 'inbox'
				? 'Inbox'
				: view === 'all'
					? 'All'
					: view === 'done'
						? 'Done'
						: view === 'cancelled'
							? 'Cancelled'
							: (folder?.title ?? 'Folder')
	);
	const rows = $derived(tasks.list.value.tasks);
	// All, Done and Cancelled span folders, so each row names the list it's in.
	const folderTitles = $derived(new Map(tasks.folders.value.map((f) => [f.id, f.title])));
	function folderOf(task: Task): string | undefined {
		if (!spansFolders(view)) return undefined;
		return task.task_folder_id === null ? 'Inbox' : folderTitles.get(task.task_folder_id);
	}
	// Rows stay put when their status changes, until the next listing; the count follows at once.
	const count = $derived(
		tasks.list.value.total - rows.filter((t) => t.status !== viewStatus(view)).length
	);

	const countLabel = $derived(
		{ incomplete: 'open', completed: 'finished', cancelled: 'cancelled' }[viewStatus(view)]
	);

	// ---- Adding -------------------------------------------------------------

	let addInput = $state<HTMLInputElement>();
	// In a view narrowed to tags, a new task starts with them, and with the
	// one priority and size the view keeps, if it keeps one.
	const newTaskTags = $derived(tags.with);
	const newTaskPriority = $derived<TaskPriority>(
		priorityParam.length === 1 ? priorityParam[0] : 'none'
	);
	const newTaskSize = $derived<TaskSize | null>(sizeParam.length === 1 ? sizeParam[0] : null);

	// ---- Rows ---------------------------------------------------------------

	let listEl = $state<HTMLUListElement>();

	function rowButtons(): HTMLElement[] {
		return [...(listEl?.querySelectorAll<HTMLElement>('[data-task-row]') ?? [])];
	}

	function focusRow(index: number) {
		const buttons = rowButtons();
		if (buttons.length === 0) return;
		buttons[Math.max(0, Math.min(buttons.length - 1, index))].focus();
	}

	function navigate(from: number, delta: number) {
		focusRow(from + delta);
	}

	function run(promise: Promise<unknown>) {
		promise.catch((error) => toasts.error(error));
	}

	function setStatus(task: Task, status: TaskStatus) {
		run(tasks.setStatus(task.workspace_id, [task.id], status));
	}

	// ---- Reveal -------------------------------------------------------------

	// Search lands here with `?task=`: switch to the view that lists the task,
	// then focus its row and briefly highlight it.
	let revealing = $state<{ id: number; view: TaskView } | null>(null);
	let highlighted = $state<number | null>(null);

	$effect(() => {
		const param = page.url.searchParams.get('task');
		if (param === null) return;
		const [w, id] = [ws, Number(param)];
		untrack(() => void reveal(w, id));
	});

	async function reveal(w: number, id: number) {
		const url = new URL(page.url);
		url.searchParams.delete('task');
		try {
			const task = await tasks.find(w, id);
			if (task === null) {
				toasts.show('Task not found.');
			} else {
				revealing = { id, view: viewOf(task) };
				url.searchParams.set('view', String(revealing.view));
				// The page of the view that lists it.
				const index = await tasks.indexIn(
					w,
					revealing.view,
					order,
					tagsParam,
					priorityParam,
					sizeParam,
					id
				);
				if (index >= limit) url.searchParams.set('page', String(Math.floor(index / limit) + 1));
				else url.searchParams.delete('page');
			}
		} catch (error) {
			toasts.error(error);
		}
		void page.goto(`${url.pathname}${url.search}`, { replaceState: true });
	}

	$effect(() => {
		const target = revealing;
		if (target === null || view !== target.view || !tasks.list.loaded) return;
		if (tasks.view !== target.view) return;
		const index = rows.findIndex((t) => t.id === target.id);
		untrack(() => {
			revealing = null;
			if (index === -1) return;
			highlighted = target.id;
			setTimeout(() => {
				if (highlighted === target.id) highlighted = null;
			}, 2000);
			void tick().then(() => {
				focusRow(index);
				rowButtons()[index]?.scrollIntoView({ block: 'nearest' });
			});
		});
	});

	// ---- Details ------------------------------------------------------------

	let detailsId = $state<number | null>(null);
	let detailsOpen = $state(false);
	// Read from the list so a save shows through while the dialog is open.
	const detailsTask = $derived(rows.find((t) => t.id === detailsId) ?? null);

	function openDetails(task: Task) {
		detailsId = task.id;
		detailsOpen = true;
	}

	// ---- Move ---------------------------------------------------------------

	const destinations = $derived<{ id: number | null; title: string }[]>([
		{ id: null, title: 'Inbox' },
		...tasks.folders.value.map((f) => ({ id: f.id, title: f.title }))
	]);

	function moveTo(task: Task, folder_id: number | null) {
		if (task.task_folder_id !== folder_id) run(tasks.move(task, folder_id));
	}

	// ---- Delete -------------------------------------------------------------

	let deleting = $state<Task | null>(null);
	let deleteOpen = $state(false);
	let deleteBusy = $state(false);

	function openDelete(task: Task) {
		deleting = task;
		deleteOpen = true;
	}

	async function confirmDelete() {
		const task = deleting;
		if (task === null) return;
		const index = rows.findIndex((t) => t.id === task.id);
		deleteBusy = true;
		try {
			await tasks.remove(task.id);
			deleteOpen = false;
			await tick();
			focusRow(index);
		} catch (error) {
			toasts.error(error);
		} finally {
			deleteBusy = false;
		}
	}

	// ---- Folders ------------------------------------------------------------

	let folderDialog = $state<{ mode: 'new' } | { mode: 'rename'; folder: TaskFolder } | null>(null);
	let folderOpen = $state(false);
	let folderTitle = $state('');
	let folderError = $state<string | null>(null);
	let folderBusy = $state(false);

	function openFolderDialog(next: NonNullable<typeof folderDialog>) {
		folderDialog = next;
		folderTitle = next.mode === 'rename' ? next.folder.title : '';
		folderError = null;
		folderOpen = true;
	}

	async function submitFolder(event: SubmitEvent) {
		event.preventDefault();
		const title = folderTitle.trim();
		if (folderDialog === null || title === '') return;
		folderBusy = true;
		try {
			if (folderDialog.mode === 'new') {
				const created = await tasks.createFolder(ws, title);
				void page.goto(viewHref(created.id));
			} else {
				await tasks.renameFolder(ws, folderDialog.folder.id, title);
			}
			folderOpen = false;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') folderError = e.message;
			else toasts.error(e);
		} finally {
			folderBusy = false;
		}
	}

	// The palette's "New folder" lands here with `?new=folder`.
	$effect(() => {
		if (page.url.searchParams.get('new') !== 'folder') return;
		untrack(() => {
			openFolderDialog({ mode: 'new' });
			const url = new URL(page.url);
			url.searchParams.delete('new');
			void page.goto(`${url.pathname}${url.search}`, { replaceState: true, keepFocus: true });
		});
	});

	const folderById = (id: number) => tasks.folders.value.find((f) => f.id === id)!;

	let removingFolder = $state<TaskFolder | null>(null);
	let removeFolderOpen = $state(false);

	function openRemoveFolder(target: TaskFolder) {
		removingFolder = target;
		removeFolderOpen = true;
	}

	async function confirmRemoveFolder() {
		const target = removingFolder;
		if (target === null) return;
		folderBusy = true;
		try {
			await tasks.deleteFolder(ws, target.id);
			removeFolderOpen = false;
			if (view === target.id) void page.goto(viewHref('inbox'), { replaceState: true });
		} catch (error) {
			toasts.error(error);
		} finally {
			folderBusy = false;
		}
	}

	// ---- Page keys ----------------------------------------------------------

	function onWindowKey(event: KeyboardEvent) {
		if (event.metaKey || event.ctrlKey || event.altKey || isTyping(event.target)) return;
		if (event.key === 'n' && addInput) {
			event.preventDefault();
			addInput.focus();
		} else if (
			(event.key === 'j' || event.key === 'ArrowDown') &&
			(event.target === document.body || event.target === null)
		) {
			event.preventDefault();
			focusRow(0);
		}
	}
</script>

<PageTitle title={`${viewName} · Tasks`} />

<svelte:window onkeydown={onWindowKey} />

<!-- The header runs across the sheet as on every page; the folders and the list sit under it. -->
<div class="px-(--page-inset)">
	<PageHeader title={viewName}>
		{#snippet meta()}
			{#if tasks.list.loaded && !folderGone}
				<span>{count} {countLabel}</span>
			{/if}
		{/snippet}
		{#snippet actions()}
			{#if !folderGone}
				<!-- Filters share the title's row with the order, as every list keeps its tools there. -->
				<TaskFilters
					tags={taskTags.list.value}
					{tagState}
					{tagHref}
					priorities={priorityParam}
					{priorityHref}
					sizes={sizeParam}
					{sizeHref}
					onmanagetags={() => (managingTags = true)}
				/>
				<SortMenu {order} onchange={setOrder} {showIds} onshowids={setShowIds} />
			{/if}
		{/snippet}
	</PageHeader>
	<div class="flex gap-6 max-md:flex-col max-md:gap-0">
		<aside class={navPane}>
			<div class="flex flex-col gap-4 max-md:gap-1">
				<FolderNav
					folders={tasks.folders.value}
					current={typeof shown === 'number' ? shown : null}
					href={viewHref}
					onnew={() => openFolderDialog({ mode: 'new' })}
					onrename={(f) => openFolderDialog({ mode: 'rename', folder: folderById(f.id) })}
					ondelete={(f) => openRemoveFolder(folderById(f.id))}
					onreorder={(ids) => tasks.reorderFolders(ws, ids).catch((e) => toasts.error(e))}
				>
					{#snippet before()}
						<!-- The lists across folders first, unlabelled as Articles' and Settings' first rows
						     are, then the folders under their label. -->
						{#each [['inbox', 'Inbox'], ['all', 'All'], ['done', 'Done'], ['cancelled', 'Cancelled']] as const as [v, label] (v)}
							<li class={navItem}>
								<a
									class={navLink}
									href={viewHref(v)}
									aria-current={shown === v ? 'page' : undefined}>{label}</a
								>
							</li>
						{/each}
						<li class={navHeading}>Folders</li>
					{/snippet}
				</FolderNav>

				<SavedViewsNav
					views={tasks.views.value}
					current={saved?.id ?? null}
					href={savedHref}
					onpick={pickSaved}
					oncreate={(name) => tasks.createView(ws, name, filters)}
					onupdate={(v) => tasks.updateView(ws, v.id, filters)}
					onrename={(v, name) => tasks.renameView(ws, v.id, name)}
					ondelete={(v) => tasks.deleteView(ws, v.id)}
					onreorder={(ids) => tasks.reorderViews(ws, ids).catch((e) => toasts.error(e))}
				/>
			</div>
		</aside>

		<!-- Picking a view fades this column alone (see the root layout). -->
		<div class="min-w-0 flex-1" data-view-query="view">
			{#if folderGone}
				<EmptyState
					message="Folder not found"
					actionLabel="Back to inbox"
					onaction={() => page.goto(viewHref('inbox'), { replaceState: true })}
				/>
			{:else}
				{#if viewStatus(view) === 'incomplete'}
					<AddTaskRow
						bind:element={addInput}
						{viewName}
						folders={tasks.folders.value}
						tags={taskTags.list.value}
						folderId={viewFolder(view)}
						tagIds={newTaskTags}
						startPriority={newTaskPriority}
						startSize={newTaskSize}
						oncreate={(title, details) => tasks.create(title, details)}
						oncreatetag={(name) => taskTags.create(ws, name)}
						ondown={() => focusRow(0)}
					/>
				{/if}

				{#if tasks.list.error && !tasks.list.loaded}
					<EmptyState {...workspaceLoadError(tasks.list.error, () => tasks.list.refetch())} />
				{:else if tasks.list.loaded}
					{#if rows.length === 0}
						{#if view === 'done'}
							<EmptyState
								message="No finished tasks yet"
								actionLabel="Show open tasks"
								onaction={() => page.goto(viewHref('all'))}
							/>
						{:else if view === 'cancelled'}
							<EmptyState
								message="No cancelled tasks"
								actionLabel="Show open tasks"
								onaction={() => page.goto(viewHref('all'))}
							/>
						{:else}
							<!-- The add field just above is the action. -->
							<EmptyState compact message="No tasks here" />
						{/if}
					{:else}
						<ul bind:this={listEl} aria-label="{viewName} tasks">
							{#each rows as task, index (task.id)}
								<TaskRow
									{task}
									highlighted={highlighted === task.id}
									folder={folderOf(task)}
									showId={showIds}
									ontoggle={() => run(tasks.toggle(task))}
									onrename={(title) => tasks.rename(task.id, title)}
									onstatus={(status) => setStatus(task, status)}
									onpriority={(priority) => run(tasks.setPriority(task, priority))}
									onschedule={(start, due) => run(tasks.schedule(task, start, due))}
									ondetails={() => openDetails(task)}
									{destinations}
									onmove={(folder_id) => moveTo(task, folder_id)}
									tags={taskTags.list.value}
									ontag={(tag_id, tagged) => run(setTagged(task, tag_id, tagged))}
									onmanagetags={() => (managingTags = true)}
									contacts={contacts.list.value}
									onassign={(assignee) => run(tasks.setAssignee(task, assignee))}
									onsize={(size) => run(tasks.setSize(task, size))}
									ondelete={() => openDelete(task)}
									onnavigate={(delta) => navigate(index, delta)}
								/>
							{/each}
						</ul>
					{/if}
					<Pager
						{offset}
						{limit}
						total={tasks.list.value.total}
						onpage={showPage}
						onsize={showSize}
					/>
				{/if}
			{/if}
		</div>
	</div>
</div>

<TagsDialog bind:open={managingTags} store={taskTags} workspaceId={ws} />

<TaskDetails
	bind:open={detailsOpen}
	task={detailsTask}
	folders={tasks.folders.value}
	tags={taskTags.list.value}
	contacts={contacts.list.value}
/>

<Dialog
	bind:open={deleteOpen}
	title="Delete “{deleting?.title ?? ''}”?"
	description="This can’t be undone."
>
	{#snippet footer()}
		<Button onclick={() => (deleteOpen = false)}>Cancel</Button>
		<Button variant="danger-quiet" disabled={deleteBusy} onclick={confirmDelete}>Delete</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={folderOpen}
	title={folderDialog?.mode === 'rename' ? 'Rename folder' : 'New folder'}
>
	<form id="task-folder-form" onsubmit={submitFolder}>
		<Field label="Title" for="task-folder-title" error={folderError}>
			<TextInput
				id="task-folder-title"
				bind:value={folderTitle}
				autocomplete="off"
				oninput={() => (folderError = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (folderOpen = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="task-folder-form"
			disabled={folderBusy || folderTitle.trim() === ''}
		>
			{folderDialog?.mode === 'rename' ? 'Rename' : 'Create'}
		</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={removeFolderOpen}
	title="Delete “{removingFolder?.title ?? ''}”?"
	description="All tasks in this folder will also be deleted."
>
	{#snippet footer()}
		<Button onclick={() => (removeFolderOpen = false)}>Cancel</Button>
		<Button variant="danger-quiet" disabled={folderBusy} onclick={confirmRemoveFolder}>
			Delete
		</Button>
	{/snippet}
</Dialog>
