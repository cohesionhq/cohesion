<!--
	An event's panel (`ItemPanel`): its title, calendar, whether it is all day,
	when it starts and ends (a day picker, and a time on the 12-hour clock in
	15-minute steps), where, who is invited and how they answered, its color,
	then its notes. Each change saves as it is made. A new event (`draft`) is
	only made once it has a title; closed without one, it is dropped. An event
	on a read-only calendar shows the same rows without controls.
-->
<script lang="ts" module>
	import type { EventTiming as Timing } from '$lib/protocol/types';

	/** A new event's starting point: where it goes and when. */
	export interface EventDraft {
		calendar_id: number;
		timing: Timing;
	}
</script>

<script lang="ts">
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import Clock from '@lucide/svelte/icons/clock';
	import MapPin from '@lucide/svelte/icons/map-pin';
	import Palette from '@lucide/svelte/icons/palette';
	import Sun from '@lucide/svelte/icons/sun';
	import Trash from '@lucide/svelte/icons/trash-2';
	import Type from '@lucide/svelte/icons/type';
	import Users from '@lucide/svelte/icons/users';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import DayPicker from '$lib/features/tasks/DayPicker.svelte';
	import { dayToDate, longDay, shortTime, ymd } from '$lib/format/dates';
	import type {
		Calendar,
		CalendarEvent,
		CalendarEventFields,
		EventInvitee,
		IntelligenceAgentColor,
		InviteeResponse
	} from '$lib/protocol/types';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { calendars, fieldsOf } from '$lib/stores/calendars.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { agentFill } from '$lib/ui/colors';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import ItemDetail, { editable } from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import Picker, { type PickerItem } from '$lib/ui/Picker.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { addDays, atMinute, daysBetween, localZone, minuteOf } from './days';

	const { calendarEvents } = pageStores();

	interface Props {
		open?: boolean;
		/** The event open, or null for a new one from `draft`. */
		event: CalendarEvent | null;
		draft: EventDraft | null;
		/** Calendars a new event can go on: those not read-only. */
		writable: Calendar[];
		/** The new event, once it is made, so the page holds it as the open one. */
		oncreated: (event: CalendarEvent) => void;
	}

	let { open = $bindable(false), event, draft, writable, oncreated }: Props = $props();

	let fields = $state<CalendarEventFields>(blank());
	let calendarId = $state(0);
	let titleInput = $state<HTMLInputElement>();
	let deleting = $state<CalendarEvent | null>(null);
	let newInvitee = $state('');

	function blank(): CalendarEventFields {
		return {
			title: '',
			location: '',
			description: '',
			timing: draft?.timing ?? {
				kind: 'allDay',
				start_date: ymd(Date.now() / 1000),
				end_date: ymd(Date.now() / 1000)
			},
			invitees: [],
			color: null
		};
	}

	// What the panel shows follows the event it opens on; a change saved comes back as the same id.
	let shownId: number | null | undefined;
	$effect.pre(() => {
		if (!open) {
			shownId = undefined;
			return;
		}
		const id = event?.id ?? null;
		if (id === shownId && id !== null) {
			// The same event, changed elsewhere (a drag, a sync): show what it says now.
			fields = fieldsOf(event!);
			return;
		}
		if (shownId !== undefined && id === null) return;
		shownId = id;
		fields = event ? fieldsOf(event) : blank();
		calendarId = event?.calendar_id ?? draft?.calendar_id ?? 0;
		newInvitee = '';
	});

	const calendar = $derived(calendars.calendar(calendarId));
	const store = $derived(calendar ? calendars.store(calendar.store_id) : undefined);
	const readOnly = $derived(calendar?.read_only ?? false);
	const color = $derived<IntelligenceAgentColor>(
		(fields.color === null
			? calendar?.color.nearest
			: fields.color.kind === 'palette'
				? fields.color.color
				: event?.color?.nearest) ?? 'blue'
	);

	/** Saves `change`: an update, or for a new event with a title, its creation. */
	async function commit(change: Partial<CalendarEventFields>) {
		const next = { ...fields, ...change };
		fields = next;
		if (readOnly) return;
		try {
			if (event) {
				await calendarEvents.update(event.id, next);
			} else if (next.title.trim() !== '' && calendarId) {
				const created = await calendarEvents.create(calendarId, {
					...next,
					title: next.title.trim()
				});
				shownId = created.id;
				oncreated(created);
			}
		} catch (error) {
			toasts.error(error);
		}
	}

	function commitText(key: 'title' | 'location' | 'description', value: string) {
		if (key === 'title' && value.trim() === '') {
			// An event keeps its title; a new one waits for one.
			if (event) fields = { ...fields, title: event.title };
			return;
		}
		if (event && event[key] === value) return;
		// A new event is made by its title; the other fields wait for it.
		if (!event && key !== 'title') return;
		void commit({ [key]: value });
	}

	// ---- Timing -------------------------------------------------------------

	function setAllDay(allDay: boolean) {
		const t = fields.timing;
		if (allDay && t.kind === 'timed')
			void commit({
				timing: {
					kind: 'allDay',
					start_date: ymd(t.starts_at),
					end_date: ymd(Math.max(t.starts_at, t.ends_at - 1))
				}
			});
		else if (!allDay && t.kind === 'allDay') {
			const starts_at = atMinute(t.start_date, 9 * 60);
			void commit({
				timing: { kind: 'timed', starts_at, ends_at: starts_at + 3600, time_zone: localZone() }
			});
		}
	}

	/** The start or end as a day and, timed, a minute. */
	function partsOf(which: 'start' | 'end'): { day: string; minute: number | null } {
		const t = fields.timing;
		if (t.kind === 'allDay')
			return { day: which === 'start' ? t.start_date : t.end_date, minute: null };
		const at = which === 'start' ? t.starts_at : t.ends_at;
		return { day: ymd(at), minute: minuteOf(at) };
	}

	/**
	 * Moves the start (keeping the length) or the end (keeping it after the
	 * start: an end before the start moves the start back by the same length).
	 */
	function setPart(which: 'start' | 'end', day: string, minute: number | null) {
		const t = fields.timing;
		if (t.kind === 'allDay') {
			if (which === 'start') {
				const length = daysBetween(t.start_date, t.end_date);
				void commit({ timing: { ...t, start_date: day, end_date: addDays(day, length) } });
			} else {
				void commit({
					timing: { ...t, end_date: day, start_date: day < t.start_date ? day : t.start_date }
				});
			}
			return;
		}
		const at = atMinute(day, minute ?? 0);
		const length = t.ends_at - t.starts_at;
		if (which === 'start') void commit({ timing: { ...t, starts_at: at, ends_at: at + length } });
		else
			void commit({
				timing:
					at >= t.starts_at ? { ...t, ends_at: at } : { ...t, starts_at: at - length, ends_at: at }
			});
	}

	const times: PickerItem[] = Array.from({ length: 96 }, (_, i) => ({
		id: i * 15,
		label: shortTime(atMinute('2026-01-01', i * 15)),
		checked: false
	}));

	// ---- Invitees -----------------------------------------------------------

	const RESPONSES: Record<InviteeResponse, string> = {
		needsAction: 'No reply',
		accepted: 'Going',
		declined: 'Not going',
		tentative: 'Maybe'
	};

	function addInvitee(e: SubmitEvent) {
		e.preventDefault();
		const email = newInvitee.trim();
		if (!/^[^\s@]+@[^\s@]+$/.test(email)) {
			toasts.error('Write an email address, like name@example.com.');
			return;
		}
		newInvitee = '';
		if (fields.invitees.some((i) => i.email.toLowerCase() === email.toLowerCase())) return;
		void commit({ invitees: [...fields.invitees, { email, response: 'needsAction' }] });
	}

	function removeInvitee(invitee: EventInvitee) {
		void commit({ invitees: fields.invitees.filter((i) => i !== invitee) });
	}

	// ---- Menus --------------------------------------------------------------

	const calendarItems = $derived<MenuItem[]>(
		writable.map((c) => ({
			id: c.id,
			label: c.title,
			checked: c.id === calendarId,
			onSelect: () => (calendarId = c.id)
		}))
	);

	const colorItems = $derived<MenuItem[]>([
		{
			label: 'Calendar’s color',
			checked: fields.color === null,
			onSelect: () => void commit({ color: null })
		},
		...vocabulary.value.agent_colors.map(({ kind, label }, i) => ({
			label,
			separated: i === 0,
			checked:
				fields.color !== null &&
				(fields.color.kind === 'palette'
					? fields.color.color === kind
					: event?.color?.nearest === kind),
			onSelect: () =>
				void commit({ color: { kind: 'palette', color: kind as IntelligenceAgentColor } })
		}))
	]);

	const colorLabel = $derived(
		fields.color === null
			? 'Calendar’s color'
			: (vocabulary.value.agent_colors.find((c) => c.kind === color)?.label ?? 'Custom')
	);

	const field =
		'-mx-2 min-h-7 w-full min-w-0 rounded-sm bg-transparent px-2 outline-none placeholder:text-fg-subtle hover:bg-bg-hover focus:bg-bg-hover disabled:cursor-default disabled:hover:bg-transparent';

	const syncNote = $derived(
		store?.backend.kind === 'account' && event?.pending_push
			? `Changes go to ${store.title} at the next sync.`
			: null
	);
</script>

{#snippet when(which: 'start' | 'end')}
	{@const part = partsOf(which)}
	<DayPicker
		single
		start={null}
		due={part.day}
		onchange={(_, day) => day && setPart(which, day, part.minute)}
	>
		{#snippet trigger(props)}
			<button
				class={editable}
				type="button"
				disabled={readOnly}
				aria-label="{which === 'start' ? 'Starts' : 'Ends'} on {longDay(part.day)}"
				{...props}
			>
				{dayToDate(part.day).toLocaleDateString(undefined, {
					weekday: 'short',
					month: 'short',
					day: 'numeric',
					year: 'numeric'
				})}
			</button>
		{/snippet}
	</DayPicker>
	{#if part.minute !== null}
		<Picker
			items={times.map((t) => ({ ...t, checked: t.id === part.minute }))}
			onpick={(minute) => setPart(which, part.day, Number(minute))}
			inDialog
			placeholder="Find a time"
			empty="No such time"
		>
			{#snippet trigger(props)}
				<button
					class={[editable, 'tabular-nums']}
					type="button"
					disabled={readOnly}
					aria-label="{which === 'start' ? 'Starts' : 'Ends'} at {shortTime(
						atMinute(part.day, part.minute!)
					)}"
					{...props}
				>
					{shortTime(atMinute(part.day, part.minute!))}
				</button>
			{/snippet}
		</Picker>
	{/if}
{/snippet}

<ItemPanel
	bind:open
	title={fields.title.trim() || (event ? event.title : 'New event')}
	icon={CalendarIcon}
	kind="Event"
	onOpenAutoFocus={(e) => {
		if (event) return;
		e.preventDefault();
		void tick().then(() => titleInput?.focus());
	}}
>
	{#snippet leading()}
		{#if readOnly}
			<span class="text-xs text-fg-subtle">{calendar?.title} is read-only.</span>
		{:else if syncNote}
			<span class="text-xs text-fg-subtle">{syncNote}</span>
		{/if}
	{/snippet}
	{#snippet actions()}
		{#if event && !readOnly}
			<button
				class="inline-flex size-7.5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
				type="button"
				aria-label="Delete event"
				onclick={() => (deleting = event)}
			>
				<Trash size={16} strokeWidth={1.75} />
			</button>
		{/if}
	{/snippet}
	{#snippet details()}
		<ItemDetail label="Title" icon={Type}>
			<input
				bind:this={titleInput}
				class={[field, 'font-medium']}
				value={fields.title}
				placeholder="New event"
				aria-label="Title"
				disabled={readOnly}
				oninput={(e) => (fields.title = e.currentTarget.value)}
				onblur={(e) => commitText('title', e.currentTarget.value)}
				onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
			/>
		</ItemDetail>
		<ItemDetail label="Calendar" icon={CalendarDays}>
			{#if event || writable.length < 2}
				<span class="inline-flex items-center gap-2">
					<span class={['size-2.5 rounded-full', calendar && agentFill[calendar.color.nearest]]}
					></span>
					{calendar?.title ?? ''}
					{#if store}<span class="text-fg-subtle">{store.title}</span>{/if}
				</span>
			{:else}
				<Menu items={calendarItems} align="start" inDialog>
					{#snippet trigger(props)}
						<button
							class={[editable, 'gap-2']}
							type="button"
							aria-label="Calendar: {calendar?.title}"
							{...props}
						>
							<span class={['size-2.5 rounded-full', calendar && agentFill[calendar.color.nearest]]}
							></span>
							{calendar?.title ?? ''}
						</button>
					{/snippet}
				</Menu>
			{/if}
		</ItemDetail>
		<ItemDetail label="All day" icon={Sun}>
			<Switch
				checked={fields.timing.kind === 'allDay'}
				onchange={setAllDay}
				disabled={readOnly}
				label="All day"
			/>
		</ItemDetail>
		<ItemDetail label="Starts" icon={Clock}>{@render when('start')}</ItemDetail>
		<ItemDetail label="Ends" icon={Clock}>{@render when('end')}</ItemDetail>
		<ItemDetail label="Location" icon={MapPin}>
			<input
				class={field}
				value={fields.location}
				placeholder="None"
				aria-label="Location"
				disabled={readOnly}
				oninput={(e) => (fields.location = e.currentTarget.value)}
				onblur={(e) => commitText('location', e.currentTarget.value)}
				onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
			/>
		</ItemDetail>
		<ItemDetail label="Invitees" icon={Users}>
			<div class="flex w-full flex-col gap-0.5 py-0.5">
				{#each fields.invitees as invitee (invitee.email)}
					<div class="group/invitee flex min-h-7 items-center gap-2">
						<span class="min-w-0 truncate">{invitee.name || invitee.email}</span>
						<span class="flex-none text-xs text-fg-subtle"
							>{RESPONSES[invitee.response ?? 'needsAction']}</span
						>
						{#if !readOnly}
							<button
								class="ml-auto inline-flex size-6 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted opacity-0 group-focus-within/invitee:opacity-100 group-hover/invitee:opacity-100 hover:bg-bg-hover hover:text-fg touch:opacity-100"
								type="button"
								aria-label="Remove {invitee.name || invitee.email}"
								onclick={() => removeInvitee(invitee)}
							>
								<X size={14} strokeWidth={1.75} />
							</button>
						{/if}
					</div>
				{/each}
				{#if !readOnly}
					<form onsubmit={addInvitee}>
						<input
							class={field}
							type="email"
							bind:value={newInvitee}
							placeholder="Add by email"
							aria-label="Invite by email"
						/>
					</form>
				{:else if fields.invitees.length === 0}
					<span class="text-fg-muted">None</span>
				{/if}
			</div>
		</ItemDetail>
		<ItemDetail label="Color" icon={Palette}>
			{#if readOnly}
				<span class="inline-flex items-center gap-2">
					<span class={['size-2.5 rounded-full', agentFill[color]]}></span>{colorLabel}
				</span>
			{:else}
				<Menu items={colorItems} align="start" inDialog>
					{#snippet trigger(props)}
						<button
							class={[editable, 'gap-2']}
							type="button"
							aria-label="Color: {colorLabel}"
							{...props}
						>
							<span class={['size-2.5 rounded-full', agentFill[color]]}></span>{colorLabel}
						</button>
					{/snippet}
				</Menu>
			{/if}
		</ItemDetail>
	{/snippet}
	<textarea
		class="min-h-40 w-full flex-1 resize-none bg-transparent leading-relaxed outline-none placeholder:text-fg-subtle"
		value={fields.description}
		placeholder={readOnly ? 'No notes' : 'Notes'}
		aria-label="Notes"
		disabled={readOnly}
		oninput={(e) => (fields.description = e.currentTarget.value)}
		onblur={(e) => commitText('description', e.currentTarget.value)}></textarea>
</ItemPanel>

<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.title ?? ''}”?"
	description={store?.backend.kind === 'account'
		? `It’s deleted from ${store.title} too, at the next sync.`
		: undefined}
	confirmLabel="Delete"
	onconfirm={async (gone) => {
		await calendarEvents.remove(gone.id);
		open = false;
	}}
/>
