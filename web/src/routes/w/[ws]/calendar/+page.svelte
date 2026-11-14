<!--
	The calendar: the events of the calendars this workspace shows (its
	perspective, or every calendar while it is bypassed) by month, week or day,
	around one day. The header steps back and forward and to today, switches the
	view, syncs the accounts and makes an event; the calendars, which this
	workspace shows and which it doesn't, sit in the column right of the sheet,
	or below its width in a panel from the header. An event opens in a panel
	(`EventPanel`); dragging across empty time or clicking a day makes one, and
	dragging an event moves it. `?view=` and `?day=` say what shows; without a
	view, the one last shown.
-->
<script lang="ts">
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import ListChecks from '@lucide/svelte/icons/list-checks';
	import Plus from '@lucide/svelte/icons/plus';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import { untrack } from 'svelte';
	import CalendarList from '$lib/features/calendar/CalendarList.svelte';
	import EventPanel, { type EventDraft } from '$lib/features/calendar/EventPanel.svelte';
	import MonthGrid from '$lib/features/calendar/MonthGrid.svelte';
	import TimeGrid from '$lib/features/calendar/TimeGrid.svelte';
	import {
		atMinute,
		eventDays,
		isDay,
		localZone,
		parseView,
		rangeOf,
		step,
		viewDays,
		viewTitle,
		VIEWS,
		type CalendarView
	} from '$lib/features/calendar/days';
	import { isDrawn } from '$lib/features/calendar/perspective.svelte';
	import { localDay } from '$lib/format/dates';
	import type { CalendarEvent, EventTiming } from '$lib/protocol/types';
	import { pane, panes } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { routes } from '$lib/shell/routes';
	import { isTyping } from '$lib/shell/shortcuts.svelte';
	import { calendars, fieldsOf } from '$lib/stores/calendars.svelte';
	import { modules } from '$lib/stores/modules.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { paneChrome } from '$lib/ui/paneChrome';
	import Segmented from '$lib/ui/Segmented.svelte';
	import SidePanel from '$lib/ui/SidePanel.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	const here = pane();
	const { calendarEvents } = pageStores();
	/** Whether the column right of the sheet can list the calendars: the main half, from 1100px. */
	const hasGutter = paneChrome()?.hasGutter ?? false;

	const ws = $derived(Number(here.params.ws));

	// A workspace with Calendar turned off has no calendar page: it opens the Dashboard instead.
	$effect(() => {
		if (modules.current.key === ws && modules.current.loaded && !modules.isEnabled('calendar'))
			void here.goto(routes.dashboard(ws), { replaceState: true });
	});

	// ---- What shows ----------------------------------------------------------

	const VIEW_KEY = 'cohesion.calendarView';
	function rememberedView(): CalendarView {
		try {
			return parseView(localStorage.getItem(VIEW_KEY)) ?? 'week';
		} catch {
			return 'week';
		}
	}

	const view = $derived(parseView(here.url.searchParams.get('view')) ?? rememberedView());
	const dayParam = $derived(here.url.searchParams.get('day'));
	const anchor = $derived(isDay(dayParam) ? dayParam : localDay());
	const days = $derived(viewDays(view, anchor));
	const range = $derived(rangeOf(days));
	const title = $derived(viewTitle(view, anchor));

	$effect(() => {
		try {
			localStorage.setItem(VIEW_KEY, view);
		} catch {
			// Private browsing: the view lasts until the page reloads.
		}
	});

	/** Shows `next` view around `day`; a step or a new view takes this entry's place. */
	function show(next: CalendarView, day: string) {
		const href = `${routes.calendar(ws)}?view=${next}${day === localDay() ? '' : `&day=${day}`}`;
		void here.goto(href, { replaceState: true, keepFocus: true });
	}

	// ---- Data ---------------------------------------------------------------

	$effect(() => {
		const id = ws;
		untrack(() => void calendars.load(id));
	});
	$effect(() => {
		const r = range;
		untrack(() => void calendarEvents.load(r));
	});
	$effect(() => () => calendarEvents.list.release());

	const shown = $derived(calendarEvents.list.value.filter((e) => isDrawn(ws, e.calendar_id)));
	const colorOf = (event: CalendarEvent) =>
		event.selected_color ?? calendars.calendar(event.calendar_id)?.selected_color ?? 'blue';
	const canEdit = (event: CalendarEvent) =>
		!(calendars.calendar(event.calendar_id)?.read_only ?? true);

	/** Calendars an event can be made on, the shown ones first. */
	const writable = $derived(
		calendars.calendars.value
			.filter((c) => !c.read_only)
			.sort((a, b) => Number(isDrawn(ws, b.id)) - Number(isDrawn(ws, a.id)))
	);

	// ---- Events -------------------------------------------------------------

	let panelOpen = $state(false);
	let openId = $state<number | null>(null);
	let draft = $state<EventDraft | null>(null);
	/** An event opened by `?event=` (a search hit), until the days around it load. */
	let linked = $state<CalendarEvent | null>(null);
	const openEvent = $derived(
		openId === null
			? null
			: (calendarEvents.list.value.find((e) => e.id === openId) ??
					(linked?.id === openId ? linked : null))
	);

	// `?event=` opens that event at its first day, then leaves the address as that day's.
	const eventParam = $derived(Number(here.url.searchParams.get('event')) || null);
	$effect(() => {
		const id = eventParam;
		if (id === null) return;
		untrack(() => {
			calendarEvents.get(id).then(
				(event) => {
					linked = event;
					openOne(event);
					show(view, eventDays(event)[0]);
				},
				(e) => {
					toasts.error(e);
					show(view, anchor);
				}
			);
		});
	});

	function openOne(event: CalendarEvent) {
		openId = event.id;
		draft = null;
		panelOpen = true;
	}

	function create(timing: EventTiming) {
		const calendar = writable[0];
		if (!calendar) {
			toasts.error('Make a calendar first: New calendar, under an account in the calendar list.');
			return;
		}
		openId = null;
		draft = { calendar_id: calendar.id, timing };
		panelOpen = true;
	}

	/** A new event from the header or `n`: the next hour of the day shown, or 9 AM on another day. */
	function createNow() {
		const today = localDay();
		const day = days.includes(today) ? today : days[0];
		const hour = day === today ? new Date().getHours() + 1 : 9;
		const starts_at = atMinute(day, Math.min(hour, 23) * 60);
		create({ kind: 'timed', starts_at, ends_at: starts_at + 3600, time_zone: localZone() });
	}

	function move(event: CalendarEvent, timing: EventTiming) {
		calendarEvents.update(event.id, { ...fieldsOf(event), timing }).catch((e) => toasts.error(e));
	}

	async function sync() {
		try {
			const summary = await calendars.sync();
			if (summary.failed > 0)
				toasts.error(
					summary.failed === 1
						? 'One account didn’t sync. The calendar list says why.'
						: `${summary.failed} accounts didn’t sync. The calendar list says why.`
				);
		} catch (e) {
			toasts.error(e);
		}
	}

	let listOpen = $state(false);

	// ---- Keys ---------------------------------------------------------------

	function onkey(event: KeyboardEvent) {
		if (panes.current !== here || panelOpen || listOpen) return;
		if (event.metaKey || event.ctrlKey || event.altKey || isTyping(event.target)) return;
		const views: Record<string, CalendarView> = { m: 'month', w: 'week', d: 'day' };
		if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
			show(view, step(view, anchor, event.key === 'ArrowLeft' ? -1 : 1));
		} else if (event.key === 't') {
			show(view, localDay());
		} else if (views[event.key]) {
			show(views[event.key], anchor);
		} else if (event.key === 'n') {
			createNow();
		} else return;
		event.preventDefault();
	}

	const tool =
		'inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg disabled:cursor-default';
</script>

<PageTitle title="{title} · Calendar" />

<svelte:window onkeydown={onkey} />

<div class="-mb-12 flex h-[calc(100%+var(--s8))] flex-col px-(--page-inset)">
	<PageHeader {title}>
		{#snippet actions()}
			<div class="flex items-center gap-0.5">
				<button
					class={tool}
					type="button"
					aria-label="Previous {view}"
					onclick={() => show(view, step(view, anchor, -1))}
				>
					<ChevronLeft size={16} strokeWidth={1.75} />
				</button>
				<Button variant="quiet" onclick={() => show(view, localDay())}>Today</Button>
				<button
					class={tool}
					type="button"
					aria-label="Next {view}"
					onclick={() => show(view, step(view, anchor, 1))}
				>
					<ChevronRight size={16} strokeWidth={1.75} />
				</button>
			</div>
			<div class="max-sm:hidden">
				<Segmented
					label="View"
					segments={VIEWS}
					value={view}
					onchange={(id) => show(id as CalendarView, anchor)}
				/>
			</div>
			<button
				class={tool}
				type="button"
				aria-label="Sync calendars"
				disabled={calendars.syncing}
				onclick={sync}
			>
				<RefreshCw
					class={calendars.syncing ? 'animate-spin motion-reduce:animate-none' : ''}
					size={16}
					strokeWidth={1.75}
				/>
			</button>
			<button
				class={[tool, hasGutter && 'lg:hidden']}
				type="button"
				aria-label="Calendars"
				onclick={() => (listOpen = true)}
			>
				<ListChecks size={16} strokeWidth={1.75} />
			</button>
			<Button variant="secondary" onclick={createNow}
				><Plus size={15} strokeWidth={1.75} aria-hidden="true" /><span class="max-sm:sr-only"
					>New event</span
				></Button
			>
		{/snippet}
	</PageHeader>

	<div class="flex min-h-0 flex-1 flex-col pb-(--page-inset)">
		<div class="mb-2 sm:hidden">
			<Segmented
				label="View"
				segments={VIEWS}
				value={view}
				onchange={(id) => show(id as CalendarView, anchor)}
			/>
		</div>
		{#if view === 'month'}
			<MonthGrid
				{days}
				month={anchor.slice(0, 7)}
				events={shown}
				{colorOf}
				{canEdit}
				onopen={openOne}
				oncreate={create}
				onmove={move}
				onday={(day) => show('day', day)}
			/>
		{:else}
			{#key view}
				<TimeGrid
					{days}
					events={shown}
					{colorOf}
					{canEdit}
					onopen={openOne}
					oncreate={create}
					onmove={move}
					onday={(day) => show('day', day)}
				/>
			{/key}
		{/if}
	</div>
</div>

<EventPanel
	bind:open={panelOpen}
	event={openEvent}
	{draft}
	{writable}
	oncreated={(event) => {
		openId = event.id;
		draft = null;
	}}
/>

<SidePanel bind:open={listOpen} title="Calendars">
	<div class="-mx-2 flex flex-col">
		<CalendarList workspaceId={ws} inDialog onleave={() => (listOpen = false)} />
	</div>
</SidePanel>
