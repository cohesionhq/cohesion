<!--
	One task, edited in place: checkbox, a title that renames, its start and
	due days (each opens a day picker; a calendar shows on hover while a day
	is unset), a notes icon that opens the description (shown on hover while
	there is none), who it is assigned to, which picks from the user, the
	agents and the workspace's contacts, its priority flag, and an overflow
	menu, whose "Move to" opens the folders beside it, "Tags" its tags,
	"Assign to" the same choices as the assignee, "Size" the sizes, and
	"Forward" the agents to send it to. The line under the title names, when the task has them, its
	#id, its size, its days, its list and its tags, in that order. Only the description opens a dialog. Views that span folders pass
	`folder` to name the task's list under its title, beside its tags;
	`showId` puts the task's #id first in that line.
	The title button is the row's keyboard focus; arrow and j/k presses go
	to the parent through `onnavigate`, and 1–4 set the priority (0 clears
	it).
-->
<script lang="ts">
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Flag from '@lucide/svelte/icons/flag';
	import NotebookText from '@lucide/svelte/icons/notebook-text';
	import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';
	import Ruler from '@lucide/svelte/icons/ruler';
	import { tick } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { forwarding } from '$lib/features/agents/forward.svelte';
	import TagPills from '$lib/features/tags/TagPills.svelte';
	import { tagsOn, tagsSubmenu } from '$lib/features/tags/tags';
	import type { Contact, Tag, Task, TaskAssignee } from '$lib/protocol/types';
	import Avatar from '$lib/ui/Avatar.svelte';
	import Checkbox from '$lib/ui/Checkbox.svelte';
	import Menu, { type MenuAction, type MenuItem } from '$lib/ui/Menu.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { copyToClipboard, toasts } from '$lib/ui/toasts.svelte';
	import AssigneeMenu from './AssigneeMenu.svelte';
	import DayPicker from './DayPicker.svelte';
	import { assigneeId, NO_ONE } from './assignee';
	import { assignees } from './assignees.svelte';
	import { PRIORITY_KEYS, priorityOf } from './priority';
	import PriorityMenu from './PriorityMenu.svelte';
	import { scheduleChips } from './schedule';
	import { sizeOf, sizes } from './size';

	interface Props {
		task: Task;
		/** Marks the row a search just led to. */
		highlighted?: boolean;
		/** The list the task is in, shown under the title. */
		folder?: string;
		/** Shows the task's ID, muted, at the start of the line under the title. */
		showId?: boolean;
		ontoggle: () => void;
		onrename: (title: string) => Promise<unknown>;
		onstatus: (status: Task['status']) => void;
		onpriority: (priority: Task['priority']) => void;
		onschedule: (start_date: string | null, due_date: string | null) => void;
		ondetails: () => void;
		/** Where the task can move to, the inbox first; "Move to" lists them. */
		destinations: { id: number | null; title: string }[];
		onmove: (folder_id: number | null) => void;
		/** The workspace's task tags; "Tags" lists them. */
		tags: Tag[];
		ontag: (tag_id: number, tagged: boolean) => void;
		onmanagetags: () => void;
		/** The workspace's contacts; "Assign to" lists them after the agents. */
		contacts: Contact[];
		onassign: (assignee: TaskAssignee | null) => void;
		onsize: (size: Task['size']) => void;
		ondelete: () => void;
		onnavigate: (delta: number) => void;
	}

	let {
		task,
		highlighted = false,
		folder,
		showId = false,
		ontoggle,
		onrename,
		onstatus,
		onpriority,
		onschedule,
		ondetails,
		destinations,
		onmove,
		tags,
		ontag,
		onmanagetags,
		contacts,
		onassign,
		onsize,
		ondelete,
		onnavigate
	}: Props = $props();

	let editing = $state(false);
	let title = $state('');
	let error = $state<string | null>(null);
	let input = $state<HTMLInputElement>();
	let button = $state<HTMLButtonElement>();

	const finished = $derived(task.status !== 'incomplete');
	const chips = $derived(scheduleChips(task));
	const tagged = $derived(tagsOn(tags, task.tag_ids));
	const assignee = $derived(assignees.shown(task.assignee, contacts));
	const priority = $derived(priorityOf(task.priority));

	$effect(() => {
		if (task.assignee?.kind === 'user') assignees.loadProfile();
	});

	const assignItems = $derived<MenuAction[]>([
		...assignees.choices(contacts).map((choice) => ({
			id: choice.id,
			label: choice.label,
			avatar: choice.avatar,
			checked: choice.id === assigneeId(task.assignee),
			onSelect: () => onassign(choice.assignee)
		})),
		{
			id: 'none',
			label: NO_ONE,
			separated: true,
			checked: task.assignee === null,
			onSelect: () => onassign(null)
		}
	]);

	const sizeItems = $derived<MenuAction[]>([
		...sizes().map((s) => ({
			id: s.value,
			label: s.label,
			ariaLabel: `Size: ${s.label}`,
			checked: s.value === task.size,
			onSelect: () => onsize(s.value)
		})),
		{ id: 'none', label: 'No size', checked: task.size === null, onSelect: () => onsize(null) }
	]);

	const items = $derived<MenuItem[]>([
		{ label: 'Details…', key: 'o', onSelect: ondetails },
		{ label: 'Copy ID', onSelect: copyId },
		forwarding.submenu(() => ({
			kind: 'task',
			id: task.id,
			workspace_id: task.workspace_id,
			title: task.title
		})),
		{
			label: 'Move to',
			separated: true,
			items: destinations.map((d) => ({
				id: d.id ?? 'inbox',
				label: d.title,
				checked: d.id === task.task_folder_id,
				onSelect: () => onmove(d.id)
			}))
		},
		tagsSubmenu(tags, task.tag_ids, ontag, onmanagetags),
		{ label: 'Assign to', items: assignItems },
		{ label: 'Size', items: sizeItems },
		task.status === 'incomplete'
			? { label: 'Cancel task', separated: true, onSelect: () => onstatus('cancelled') }
			: { label: 'Reopen', separated: true, onSelect: () => onstatus('incomplete') },
		{ label: 'Delete…', key: 'backspace', danger: true, separated: true, onSelect: ondelete }
	]);

	// The bare number, as the CLI's task commands take it.
	function copyId() {
		copyToClipboard(String(task.id), 'Copied the task ID.');
	}

	async function startEdit() {
		title = task.title;
		error = null;
		editing = true;
		await tick();
		input?.focus();
		input?.select();
	}

	async function stopEdit() {
		editing = false;
		error = null;
		await tick();
		button?.focus();
	}

	async function save() {
		if (!editing) return;
		const next = title.trim();
		if (next === task.title) return stopEdit();
		// The daemon accepts an empty task title, so refuse it here.
		if (next === '') {
			error = 'A title is required.';
			input?.focus();
			return;
		}
		try {
			await onrename(next);
			await stopEdit();
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') error = failure.message;
			else toasts.error(failure);
			input?.focus();
		}
	}

	function onEditKey(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			void stopEdit();
		}
	}

	function onKey(event: KeyboardEvent) {
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		switch (event.key) {
			case 'j':
			case 'ArrowDown':
				onnavigate(1);
				break;
			case 'k':
			case 'ArrowUp':
				onnavigate(-1);
				break;
			case ' ':
				ontoggle();
				break;
			case 'Enter':
				void startEdit();
				break;
			case 'o':
				ondetails();
				break;
			case 'Delete':
			case 'Backspace':
				ondelete();
				break;
			default: {
				const priority = PRIORITY_KEYS[event.key];
				if (priority === undefined) return;
				onpriority(priority);
			}
		}
		event.preventDefault();
	}

	// What the task carries shows on a second line; the checkbox and its days then sit on the first.
	const twoLines = $derived(
		task.size !== null ||
			showId ||
			folder !== undefined ||
			tagged.length > 0 ||
			task.description !== '' ||
			assignee ||
			task.priority !== 'none'
	);
</script>

<li
	class={[
		'group/row flex min-h-11 items-center gap-3 border-border py-1.5 transition-colors duration-700 not-first:border-t',
		highlighted && 'bg-accent-soft'
	]}
>
	<span class={['flex flex-none', twoLines && 'mt-0.5 self-start']}>
		<Checkbox
			round
			checked={finished}
			aria-label={finished ? `Reopen ${task.title}` : `Complete ${task.title}`}
			onchange={ontoggle}
		/>
	</span>
	{#if editing}
		<div class="min-w-0 flex-1">
			<TextInput
				bind:value={title}
				bind:element={input}
				aria-label="Task title"
				aria-invalid={error !== null}
				autocomplete="off"
				onkeydown={onEditKey}
				onblur={save}
				oninput={() => (error = null)}
			/>
			{#if error}<p class="mt-1 text-xs text-danger">{error}</p>{/if}
		</div>
	{:else}
		<!-- The title, and under it what the task carries: its ID and size, then its days, where it is filed and its tags. -->
		<div class="min-w-0 flex-1">
			<button
				class={[
					'block w-full cursor-text text-left wrap-anywhere',
					finished ? 'text-fg-subtle' : 'text-fg'
				]}
				type="button"
				data-task-row
				bind:this={button}
				onclick={startEdit}
				onkeydown={onKey}
				onkeyup={(event) => event.key === ' ' && event.preventDefault()}
			>
				<span class={['block', finished && 'line-through']}>{task.title}</span>
			</button>
			{#if twoLines}
				<div class="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-fg-subtle">
					{#if showId}<span>#{task.id}</span>{/if}
					{#if task.size !== null}
						{@const size = sizeOf(task.size)}
						<span
							class={[
								'inline-flex h-4.5 flex-none items-center gap-1 rounded-full px-1.75 text-2xs leading-none font-medium',
								size.chip
							]}
						>
							<Ruler size={10} strokeWidth={2} aria-hidden="true" />
							<span class="sr-only">Size:</span>
							{size.label}
						</span>
					{/if}
					<!-- What is set reads in the line, in words; the row's buttons that change it show on hover. -->
					{#if task.priority !== 'none'}
						<span class={['inline-flex items-center gap-1', priority.tone]}>
							<Flag size={12} strokeWidth={2} aria-hidden="true" />{priority.label}
						</span>
					{/if}
					{#if assignee}
						<span class="inline-flex items-center gap-1">
							<Avatar who={assignee.avatar} />{assignee.label}
						</span>
					{/if}
					{#if folder !== undefined}<span>{folder}</span>{/if}
					<TagPills tags={tagged} />
					<!-- A task with a description says so in its line, in a word as everything else there is,
					     as the trailing details button shows only on hover. -->
					{#if task.description !== ''}
						<span class="inline-flex items-center gap-1">
							<NotebookText
								class="flex-none"
								size={12}
								strokeWidth={1.75}
								aria-hidden="true"
							/>Notes
						</span>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
	<!-- At rest, a task's days at the row's trailing edge, where every list keeps its dates; on hover
	     or focus (and always on a touch screen, beside them) the row's controls take their place. -->
	{#if chips.length > 0}
		<span
			class:self-start={twoLines}
			class:pt-px={twoLines}
			class="flex flex-none items-center gap-3 text-xs whitespace-nowrap group-focus-within/row:hidden group-hover/row:hidden has-[~div_[data-state=open]]:hidden touch:flex"
		>
			{#each chips as chip (chip.day)}
				<span
					class={[
						// Words alone, in one weight, as the Dashboard's Due says them: past due red, today at
						// full strength, so orange stays for what needs the user.
						chip.tone === 'overdue' && 'text-danger',
						chip.tone === 'today' && 'text-fg',
						chip.tone === 'upcoming' && 'text-fg-muted',
						chip.tone === 'muted' && 'text-fg-subtle'
					]}
				>
					{chip.label}
				</span>
			{/each}
		</span>
	{/if}
	<div
		class="hidden flex-none items-center group-focus-within/row:flex group-hover/row:flex has-data-[state=open]:flex touch:flex"
	>
		<DayPicker start={task.start_date} due={task.due_date} initial="due" onchange={onschedule}>
			{#snippet trigger(props)}
				<button
					class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-subtle opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:opacity-100 touch:opacity-100"
					type="button"
					aria-label="Days of {task.title}"
					{...props}
				>
					<CalendarIcon size={15} strokeWidth={1.75} />
				</button>
			{/snippet}
		</DayPicker>
		<button
			class={[
				'inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-subtle opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 hover:bg-bg-hover hover:text-fg touch:opacity-100'
			]}
			type="button"
			aria-label="Details of {task.title}"
			onclick={ondetails}
		>
			<PanelRightOpen size={15} strokeWidth={1.75} />
		</button>
		<AssigneeMenu {assignee} title={task.title} items={assignItems} />
		<PriorityMenu priority={task.priority} title={task.title} onchange={onpriority} />
		<span
			class="opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 has-data-[state=open]:opacity-100 touch:opacity-100"
		>
			<Menu {items}>
				{#snippet trigger(props)}
					<button
						class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-muted hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg"
						type="button"
						aria-label="More for {task.title}"
						{...props}
					>
						<Ellipsis size={16} strokeWidth={1.75} />
					</button>
				{/snippet}
			</Menu>
		</span>
	</div>
</li>
