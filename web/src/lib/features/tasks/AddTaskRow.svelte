<!--
	The Tasks page's add field. While it is focused, or holds anything, a
	strip under it chooses the new task's folder, priority, size, tags and
	days, each but the size also on an ⌥ key from the field (see PICKER_KEYS), and opens a
	description under it (⌥O), where Enter starts a new line and ⌘/Ctrl+Enter
	adds. Enter adds the task with all of it in one go and starts the row
	over; Escape, or Backspace in an empty field, starts it over without
	adding.
-->
<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import Flag from '@lucide/svelte/icons/flag';
	import Folder from '@lucide/svelte/icons/folder';
	import Inbox from '@lucide/svelte/icons/inbox';
	import NotebookText from '@lucide/svelte/icons/notebook-text';
	import Ruler from '@lucide/svelte/icons/ruler';
	import TagIcon from '@lucide/svelte/icons/tag';
	import { tick } from 'svelte';
	import { slide } from 'svelte/transition';
	import { tagsOn } from '$lib/features/tags/tags';
	import { localDay, shortDay } from '$lib/format/dates';
	import type { Tag, TaskFolder, TaskPriority, TaskSize } from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { NewTaskDetails } from '$lib/stores/tasks.svelte';
	import Picker, { type PickerItem } from '$lib/ui/Picker.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import { motion } from '$lib/ui/motion';
	import { toasts } from '$lib/ui/toasts.svelte';
	import DayPicker from './DayPicker.svelte';
	import { priorities, PRIORITY_KEYS, priorityOf } from './priority';
	import { NO_SIZE_KEY, sizeOf, sizes } from './size';

	interface Props {
		/** Names the list in the field's label. */
		viewName: string;
		folders: TaskFolder[];
		tags: Tag[];
		/** The folder a new task starts in; `null` is the inbox. */
		folderId: number | null;
		/** The tags a new task starts with. */
		tagIds: number[];
		/** The priority a new task starts with. */
		startPriority?: TaskPriority;
		/** The size a new task starts with; none when `null`. */
		startSize?: TaskSize | null;
		oncreate: (title: string, details: NewTaskDetails) => Promise<unknown>;
		oncreatetag: (name: string) => Promise<Tag>;
		/** ArrowDown in the field: move to the list. */
		ondown: () => void;
		element?: HTMLInputElement;
	}

	let {
		viewName,
		folders,
		tags,
		folderId,
		tagIds,
		startPriority = 'none',
		startSize = null,
		oncreate,
		oncreatetag,
		ondown,
		element = $bindable()
	}: Props = $props();

	let title = $state('');
	let folder = $state<number | null>(null);
	let priority = $state<TaskPriority>('none');
	let size = $state<TaskSize | null>(null);
	let chosenTags = $state<number[]>([]);
	let start = $state<string | null>(null);
	let due = $state<string | null>(null);
	let description = $state('');
	/** Whether the description's field is showing. */
	let describing = $state(false);
	let notes = $state<HTMLTextAreaElement>();
	let adding = $state(false);

	const open = $state({
		folder: false,
		priority: false,
		size: false,
		tags: false,
		start: false,
		due: false
	});

	/** In the field, ⌥ and these open a picker; ⌥ and 1–4 or 0 set the priority. */
	const PICKER_KEYS: Record<string, Exclude<keyof typeof open, 'size'>> = {
		f: 'folder',
		p: 'priority',
		t: 'tags',
		s: 'start',
		d: 'due'
	};
	let focused = $state(false);

	function reset() {
		title = '';
		folder = folderId;
		priority = startPriority;
		size = startSize;
		chosenTags = [...tagIds];
		start = null;
		due = null;
		description = '';
		describing = false;
	}

	// The view's folder, tags, priority and size are where a row starts, and it starts over when they change.
	$effect(() => {
		void folderId;
		void tagIds;
		void startPriority;
		void startSize;
		reset();
	});

	const changed = $derived(
		title !== '' ||
			folder !== folderId ||
			priority !== startPriority ||
			size !== startSize ||
			start !== null ||
			due !== null ||
			description !== '' ||
			chosenTags.length !== tagIds.length ||
			chosenTags.some((id) => !tagIds.includes(id))
	);
	const expanded = $derived(focused || changed || Object.values(open).some(Boolean));

	const today = $derived(expanded ? localDay() : '');
	const folderName = $derived(
		folder === null ? 'Inbox' : (folders.find((f) => f.id === folder)?.title ?? 'Inbox')
	);
	const tagNames = $derived(
		tagsOn(tags, chosenTags)
			.map((t) => t.name)
			.join(', ')
	);
	const current = $derived(priorityOf(priority));
	const days = $derived([
		{ which: 'start', label: 'Start', day: start },
		{ which: 'due', label: 'Due', day: due }
	] as const);

	function dayLabel(which: 'Start' | 'Due', day: string | null): string {
		if (day === null) return which;
		return day === today ? `${which} today` : `${which} ${shortDay(day, today)}`;
	}

	const folderItems = $derived<PickerItem[]>([
		{ id: 'inbox', label: 'Inbox', checked: folder === null },
		...folders.map((f) => ({ id: f.id, label: f.title, checked: folder === f.id }))
	]);
	const priorityItems = $derived<PickerItem[]>(
		priorities().map((p) => ({
			id: p.value,
			label: p.label,
			key: p.key,
			checked: p.value === priority
		}))
	);
	const sizeItems = $derived<PickerItem[]>([
		...sizes().map((s) => ({ id: s.value, label: s.label, key: s.key, checked: s.value === size })),
		{ id: 'none', label: 'No size', key: NO_SIZE_KEY, checked: size === null }
	]);
	const tagItems = $derived<PickerItem[]>(
		tags.map((t) => ({ id: t.id, label: t.name, checked: chosenTags.includes(t.id) }))
	);

	function toggleTag(id: number) {
		chosenTags = chosenTags.includes(id) ? chosenTags.filter((t) => t !== id) : [...chosenTags, id];
	}

	const createTag = {
		label: (name: string) => `Create tag “${name}”`,
		run: async (name: string) => {
			const tag = await oncreatetag(name);
			chosenTags = [...chosenTags, tag.id];
		}
	};

	// The row holds attention while focus is in it or in one of its pickers,
	// which sit in a portal; a press anywhere else lets it go. A picker from
	// elsewhere (the task panel's) is not the row's, so it counts only while
	// one of the row's own is open.
	let row = $state<HTMLDivElement>();
	function within(target: EventTarget | null): boolean {
		return (
			target instanceof Element &&
			(row?.contains(target) === true ||
				(Object.values(open).some(Boolean) && target.closest('[data-picker]') !== null))
		);
	}

	async function add() {
		const name = title.trim();
		if (name === '' || adding) return;
		adding = true;
		try {
			await oncreate(name, {
				folder_id: folder,
				priority,
				size,
				tag_ids: chosenTags,
				start_date: start,
				due_date: due,
				description: description.trimEnd()
			});
			const fromNotes = document.activeElement === notes;
			reset();
			if (fromNotes) element?.focus();
		} catch (error) {
			toasts.error(error);
		} finally {
			adding = false;
		}
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.altKey && !event.metaKey && !event.ctrlKey) {
			// By code, since ⌥ and a letter type a symbol on a Mac.
			const key = event.code.replace(/^(Key|Digit)/, '').toLowerCase();
			const picker = PICKER_KEYS[key];
			if (key === 'o') {
				event.preventDefault();
				void describe();
			} else if (picker !== undefined) {
				event.preventDefault();
				open[picker] = true;
			} else if (key in PRIORITY_KEYS) {
				event.preventDefault();
				priority = PRIORITY_KEYS[key];
			}
			return;
		}
		if (event.key === 'ArrowDown') {
			event.preventDefault();
			ondown();
		} else if (event.key === 'Enter') {
			event.preventDefault();
			void add();
		} else if ((event.key === 'Escape' || (event.key === 'Backspace' && title === '')) && changed) {
			event.preventDefault();
			reset();
		}
	}

	async function describe() {
		describing = true;
		await tick();
		notes?.focus();
	}

	/** In the description: ⌘/Ctrl+Enter adds; Escape goes back to the title, closing an empty one. */
	function onNotesKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void add();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			if (description.trim() === '') {
				description = '';
				describing = false;
			}
			element?.focus();
		}
	}

	const chip =
		'inline-flex h-6 max-w-48 cursor-pointer items-center gap-1 rounded-sm px-1.5 text-xs whitespace-nowrap hover:bg-bg-hover data-[state=open]:bg-bg-hover [&_svg]:flex-none';
	const set = 'text-fg';
	const unset = 'text-fg-subtle hover:text-fg';
</script>

<svelte:document
	onfocusin={(event) => (focused = within(event.target))}
	onpointerdown={(event) => {
		if (!within(event.target)) focused = false;
	}}
/>

<div class="mb-3" bind:this={row}>
	<TextInput
		bind:value={title}
		bind:element
		placeholder="Add a task"
		aria-label="Add a task to {viewName}"
		autocomplete="off"
		{onkeydown}
	/>
	{#if expanded}
		<div
			class="flex flex-wrap items-center gap-1 pt-1"
			role="group"
			aria-label="New task details"
			transition:slide={{ duration: motion.duration }}
		>
			<Picker
				items={folderItems}
				onpick={(id) => (folder = id === 'inbox' ? null : Number(id))}
				placeholder="Find a folder"
				empty="No folders yet."
				bind:open={open.folder}
				returnTo={element}
			>
				{#snippet trigger(props)}
					<button class={[chip, set]} type="button" aria-label="Folder: {folderName}" {...props}>
						{#if folder === null}
							<Inbox size={13} strokeWidth={1.75} />
						{:else}
							<Folder size={13} strokeWidth={1.75} />
						{/if}
						<span class="truncate">{folderName}</span>
					</button>
				{/snippet}
			</Picker>
			<Picker
				items={priorityItems}
				onpick={(id) => (priority = id as TaskPriority)}
				placeholder="Find a priority"
				empty="No matches."
				bind:open={open.priority}
				returnTo={element}
			>
				{#snippet trigger(props)}
					<button
						class={[chip, priority === 'none' ? unset : current.tone]}
						type="button"
						aria-label="Priority: {current.label}"
						{...props}
					>
						<Flag
							size={13}
							strokeWidth={1.75}
							fill={priority === 'none' ? 'none' : 'currentColor'}
						/>
						{priority === 'none' ? 'Priority' : current.label}
					</button>
				{/snippet}
			</Picker>
			<Picker
				items={sizeItems}
				onpick={(id) => (size = id === 'none' ? null : (id as TaskSize))}
				placeholder="Find a size"
				empty="No matches."
				bind:open={open.size}
				returnTo={element}
			>
				{#snippet trigger(props)}
					<button
						class={[chip, size === null ? unset : sizeOf(size).tone]}
						type="button"
						aria-label={size === null ? 'Size' : `Size: ${vocabulary.size(size)}`}
						{...props}
					>
						<Ruler size={13} strokeWidth={1.75} />
						{size === null ? 'Size' : vocabulary.size(size)}
					</button>
				{/snippet}
			</Picker>
			<Picker
				items={tagItems}
				onpick={(id) => toggleTag(Number(id))}
				multiple
				placeholder="Find or create a tag"
				empty="No tags yet."
				create={createTag}
				bind:open={open.tags}
				returnTo={element}
			>
				{#snippet trigger(props)}
					<button
						class={[chip, chosenTags.length === 0 ? unset : set]}
						type="button"
						aria-label={chosenTags.length === 0 ? 'Tags' : `Tags: ${tagNames}`}
						{...props}
					>
						<TagIcon size={13} strokeWidth={1.75} />
						<span class="truncate">{chosenTags.length === 0 ? 'Tags' : tagNames}</span>
					</button>
				{/snippet}
			</Picker>
			{#each days as d (d.which)}
				<DayPicker
					{start}
					{due}
					initial={d.which}
					onchange={(s, e) => {
						start = s;
						due = e;
					}}
					bind:open={open[d.which]}
					returnTo={element}
				>
					{#snippet trigger(props)}
						<button class={[chip, d.day === null ? unset : set]} type="button" {...props}>
							<CalendarDays size={13} strokeWidth={1.75} />
							{dayLabel(d.label, d.day)}
						</button>
					{/snippet}
				</DayPicker>
			{/each}
			<button
				class={[chip, description === '' ? unset : set]}
				type="button"
				aria-expanded={describing}
				onclick={describe}
			>
				<NotebookText size={13} strokeWidth={1.75} />
				Description
			</button>
		</div>
		{#if describing}
			<div class="pt-1" transition:slide={{ duration: motion.duration }}>
				<Textarea
					bind:value={description}
					bind:ref={notes}
					rows={3}
					placeholder="Notes, links, steps…"
					aria-label="Description"
					onkeydown={onNotesKeydown}
				/>
			</div>
		{/if}
	{/if}
</div>
