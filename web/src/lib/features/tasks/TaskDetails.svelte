<!--
	A task's panel (`ItemPanel`): where it stands, who it is assigned to, its
	priority, size, days, folder and tags, each changed in place from its row
	(a menu, a picker for the assignee, one for the tags, which stays open to
	choose several, the size
	slider, or the day picker), then its description the way a document page holds its text:
	read as Markdown, or written in the documents' editor after Edit, saving
	as it goes. It opens editing when there is no description yet. Done
	goes back to reading; ⌘/Ctrl+Enter starts editing and ends it; closing saves what is typed.
	It shows each change from the reply, so a task that leaves the page's
	list, or a linked one no list holds, stays current while it is open.
-->
<script lang="ts">
	import Calendar from '@lucide/svelte/icons/calendar';
	import CalendarClock from '@lucide/svelte/icons/calendar-clock';
	import CheckSquare from '@lucide/svelte/icons/square-check';
	import CircleDot from '@lucide/svelte/icons/circle-dot';
	import Flag from '@lucide/svelte/icons/flag';
	import Folder from '@lucide/svelte/icons/folder';
	import Ruler from '@lucide/svelte/icons/ruler';
	import UserRound from '@lucide/svelte/icons/user-round';
	import TagIcon from '@lucide/svelte/icons/tag';
	import { tick, untrack } from 'svelte';
	import DocumentBody from '$lib/features/documents/DocumentBody.svelte';
	import SavedMark from '$lib/features/documents/SavedMark.svelte';
	import { useAutosave } from '$lib/features/documents/autosave.svelte';
	import TagPills from '$lib/features/tags/TagPills.svelte';
	import { tagsOn } from '$lib/features/tags/tags';
	import { longDay } from '$lib/format/dates';
	import type { Contact, Tag, Task, TaskFolder } from '$lib/protocol/types';
	import { Draft } from '$lib/stores/draft.svelte';

	import Avatar from '$lib/ui/Avatar.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ItemDetail, { editable } from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import Picker, { type PickerItem } from '$lib/ui/Picker.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import DayPicker from './DayPicker.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import RewindButton from '$lib/features/rewind/RewindButton.svelte';
	import type { Restored } from '$lib/stores/rewind.svelte';
	import { assigneeId, NO_ONE } from './assignee';
	import { assignees } from './assignees.svelte';
	import { priorities, priorityOf } from './priority';
	import { STATUSES } from './status';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { taskTags, tasks } = pageStores();

	interface Props {
		open?: boolean;
		task: Task | null;
		/** The folders of the task's workspace; "Folder" lists them after the inbox. */
		folders: TaskFolder[];
		/** The task tags of the task's workspace; "Tags" lists them. */
		tags: Tag[];
		/** The contacts of the task's workspace; "Assignee" lists them after the agents. */
		contacts: Contact[];
	}

	let { open = $bindable(false), task: given, folders, tags, contacts }: Props = $props();

	/** The last task given, kept when the page's list lets go of it, and overridden by each reply. */
	let kept: Task | null = null;
	let task = $derived.by(() => {
		if (given) kept = given;
		return kept;
	});

	function took(changed: Task | undefined) {
		if (!changed) return;
		kept = changed;
		task = changed;
	}

	/** Takes what a restore left, description included: it replaces what the editor holds. */
	function restored(result: Restored) {
		if (result.kind !== 'task') return;
		took(result.task);
		draft.reset(result.task.description);
	}

	function run(change: Promise<Task | undefined>) {
		change.then(took, (error) => toasts.error(error));
	}

	// What each row shows; the rows render only while there is a task.
	const status = $derived(task ? STATUSES[task.status] : '');
	const priority = $derived(priorityOf(task?.priority ?? 'none'));
	const folder = $derived(folders.find((f) => f.id === task?.task_folder_id)?.title ?? 'Inbox');
	const tagged = $derived(task ? tagsOn(tags, task.tag_ids) : []);

	const STATUS_KEYS: Record<Task['status'], string> = {
		incomplete: 'o',
		completed: 'd',
		cancelled: 'c'
	};

	const statusItems = $derived<MenuItem[]>(
		(Object.keys(STATUSES) as Task['status'][]).map((value) => ({
			id: value,
			label: STATUSES[value],
			key: STATUS_KEYS[value],
			checked: value === task?.status,
			onSelect: () =>
				task &&
				value !== task.status &&
				run(tasks.setStatus(task.workspace_id, [task.id], value).then(([t]) => t))
		}))
	);

	const priorityItems = $derived<MenuItem[]>(
		priorities().map((p) => ({
			id: p.value,
			label: p.label,
			key: p.key,
			checked: p.value === task?.priority,
			onSelect: () => task && p.value !== task.priority && run(tasks.setPriority(task, p.value))
		}))
	);

	const folderItems = $derived<MenuItem[]>(
		[{ id: null, title: 'Inbox' }, ...folders].map((f) => ({
			id: f.id ?? 'inbox',
			label: f.title,
			checked: f.id === task?.task_folder_id,
			onSelect: () => task && f.id !== task.task_folder_id && run(tasks.move(task, f.id))
		}))
	);

	const assignee = $derived(task ? assignees.shown(task.assignee, contacts) : null);
	const choices = $derived(assignees.choices(contacts));
	const assigneeItems = $derived<PickerItem[]>([
		...choices.map((choice) => ({
			id: choice.id,
			label: choice.label,
			avatar: choice.avatar,
			checked: choice.id === assigneeId(task?.assignee ?? null)
		})),
		{ id: 'none', label: NO_ONE, checked: task?.assignee === null }
	]);

	function assign(id: PickerItem['id']) {
		if (!task || id === assigneeId(task.assignee)) return;
		run(tasks.setAssignee(task, choices.find((c) => c.id === id)?.assignee ?? null));
	}

	// The user's choice and avatar are drawn from their profile.
	$effect(() => {
		if (open) assignees.loadProfile();
	});

	const tagItems = $derived<PickerItem[]>(
		tags.map((t) => ({ id: t.id, label: t.name, checked: task?.tag_ids.includes(t.id) ?? false }))
	);

	async function setTagged(of: Task, tag_id: number, tagged: boolean): Promise<Task> {
		const task = await taskTags.setTagged(of.workspace_id, tag_id, of.id, tagged);
		tasks.takeTagged(task);
		return task;
	}

	/** The two day rows, each opening the day picker on its own day. */
	const days = $derived(
		task
			? ([
					{ day: 'start', label: 'Starts', icon: Calendar, value: task.start_date },
					{ day: 'due', label: 'Due', icon: CalendarClock, value: task.due_date }
				] as const)
			: []
	);

	function schedule(start: string | null, due: string | null) {
		if (task) run(tasks.schedule(task, start, due));
	}

	const now = () => Math.floor(Date.now() / 1000);

	/** The task the draft is of; saves go to it in its own workspace. */
	let subject: Task | null = null;
	const draft = new Draft(async (content) => {
		await tasks.describe(subject!, content);
		return now();
	});
	const save = useAutosave(draft);

	let preview = $state(true);
	let body = $state<DocumentBody>();
	/** The task the panel was last opened on, set as it opens; null once closed. */
	let shownId: number | null = null;

	// Start from the task's saved copy each time the panel opens on a task,
	// once the last one's edits are saved; a refetch while it is open lands
	// unless there are unsaved edits. Closing saves.
	$effect(() => {
		const [isOpen, current] = [open, task];
		untrack(() => {
			if (!isOpen) {
				if (shownId !== null) save();
				shownId = null;
			} else if (current && current.id !== shownId) {
				shownId = current.id;
				void start(current);
			} else if (current && current.id === subject?.id && current.description !== draft.saved) {
				draft.receive(current.description, now());
			}
		});
	});

	async function start(next: Task) {
		await draft.flush().catch(() => undefined);
		subject = next;
		draft.reset(next.description);
		preview = next.description !== '';
		if (!preview) await edit();
	}

	async function edit() {
		preview = false;
		await tick();
		body?.focus();
	}

	// The chord that ends editing (see `onsubmit` below) also starts it.
	function onkey(event: KeyboardEvent) {
		if (!open || !preview || event.key !== 'Enter' || !(event.metaKey || event.ctrlKey)) return;
		event.preventDefault();
		void edit();
	}

	function done() {
		preview = true;
		save();
	}

	/** A link into the app goes to its page, or opens its task in this panel's place, so the panel gets out of the way. */
	function onLink(event: MouseEvent) {
		const link = event.target instanceof Element ? event.target.closest('a') : null;
		if (link && link.origin === location.origin) open = false;
	}
</script>

<svelte:window onkeydown={onkey} />

<ItemPanel
	bind:open
	title={task?.title ?? ''}
	icon={CheckSquare}
	kind="Task #{task?.id ?? ''}"
	link={task ? { kind: 'task', ws: task.workspace_id, id: task.id } : undefined}
	onOpenAutoFocus={(event) => {
		// An empty description opens in the editor, which takes the focus itself.
		if (task?.description === '') event.preventDefault();
	}}
>
	{#snippet actions()}
		<SavedMark {draft} />
		{#if task}
			<RewindButton
				item={{ kind: 'task', workspace_id: task.workspace_id, id: task.id }}
				title={task.title}
				onopen={save}
				onrestored={restored}
			/>
		{/if}
	{/snippet}
	{#snippet leading()}
		{#if preview}
			<Button onclick={edit}>Edit</Button>
		{:else}
			<Button onclick={done}>Done</Button>
		{/if}
	{/snippet}
	{#snippet details()}
		{#if task}
			<ItemDetail label="Status" icon={CircleDot}>
				<Menu items={statusItems} align="start" inDialog>
					{#snippet trigger(props)}
						<button class={editable} type="button" aria-label="Status: {status}" {...props}>
							{status}
						</button>
					{/snippet}
				</Menu>
			</ItemDetail>
			<ItemDetail label="Assignee" icon={UserRound}>
				<Picker
					items={assigneeItems}
					onpick={assign}
					inDialog
					placeholder="Find someone"
					empty="No one to assign."
				>
					{#snippet trigger(props)}
						<button
							class={[editable, 'gap-2', !assignee && 'text-fg-muted']}
							type="button"
							aria-label="Assignee: {assignee?.label ?? NO_ONE}"
							{...props}
						>
							{#if assignee}<Avatar who={assignee.avatar} />{assignee.label}{:else}{NO_ONE}{/if}
						</button>
					{/snippet}
				</Picker>
			</ItemDetail>
			<ItemDetail label="Priority" icon={Flag}>
				<Menu items={priorityItems} align="start" inDialog>
					{#snippet trigger(props)}
						<button
							class={[editable, priority.tone]}
							type="button"
							aria-label="Priority: {priority.label}"
							{...props}
						>
							{priority.label}
						</button>
					{/snippet}
				</Menu>
			</ItemDetail>
			<ItemDetail label="Size" icon={Ruler}>
				<SizeSlider
					size={task.size}
					title={task.title}
					onchange={(size) => task && run(tasks.setSize(task, size))}
				/>
			</ItemDetail>
			{#each days as row (row.day)}
				<ItemDetail label={row.label} icon={row.icon}>
					<DayPicker
						start={task.start_date}
						due={task.due_date}
						initial={row.day}
						onchange={schedule}
					>
						{#snippet trigger(props)}
							<button
								class={[editable, row.value === null && 'text-fg-muted']}
								type="button"
								aria-label="{row.label}: {row.value ? longDay(row.value) : 'None'}"
								{...props}
							>
								{row.value ? longDay(row.value) : 'None'}
							</button>
						{/snippet}
					</DayPicker>
				</ItemDetail>
			{/each}
			<ItemDetail label="Folder" icon={Folder}>
				<Menu items={folderItems} align="start" inDialog>
					{#snippet trigger(props)}
						<button class={editable} type="button" aria-label="Folder: {folder}" {...props}>
							{folder}
						</button>
					{/snippet}
				</Menu>
			</ItemDetail>
			<ItemDetail label="Tags" icon={TagIcon}>
				{#if tags.length}
					<Picker
						items={tagItems}
						onpick={(id) =>
							task && run(setTagged(task, Number(id), !task.tag_ids.includes(Number(id))))}
						multiple
						inDialog
						placeholder="Find a tag"
						empty="No tags"
						emptyDetail="Create tags in the filter menu."
					>
						{#snippet trigger(props)}
							<button
								class={[editable, 'flex-wrap py-1', tagged.length === 0 && 'text-fg-muted']}
								type="button"
								aria-label="Tags: {tagged.map((t) => t.name).join(', ') || 'None'}"
								{...props}
							>
								{#if tagged.length}<TagPills tags={tagged} />{:else}None{/if}
							</button>
						{/snippet}
					</Picker>
				{:else}
					<span class="text-fg-muted">None</span>
				{/if}
			</ItemDetail>
		{/if}
	{/snippet}
	<!-- Only a link's own click matters here, and links are reached by keyboard themselves. -->
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="flex flex-1 flex-col" onclick={preview ? onLink : undefined}>
		<DocumentBody
			bind:this={body}
			{draft}
			workspace={task?.workspace_id ?? 0}
			{preview}
			label="Description"
			placeholder="Notes, links, steps…"
			fill
			onblur={save}
			onsubmit={done}
		/>
	</div>
</ItemPanel>
