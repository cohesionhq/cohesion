<!--
	The week and day views: a column per day under its heading, a row of
	all-day events (and timed ones spanning days) as bars, then the day's hours,
	each timed event placed by its minutes and overlapping ones side by side.
	Dragging across empty time makes an event of that range (a click, of half
	an hour); clicking the all-day row makes an all-day event that day. An
	event drags to another time or day, by 15 minutes, or a bar by days;
	a click opens it. Opens scrolled to the morning.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { localDay } from '$lib/format/dates';
	import type { CalendarEvent, EventTiming, IntelligenceAgentColor } from '$lib/protocol/types';
	import { agentFill, agentWash } from '$lib/ui/colors';
	import {
		atMinute,
		dayStart,
		hourLabel,
		layoutBars,
		layoutDay,
		localZone,
		minuteOf,
		moveByDays,
		moveTo,
		snap,
		timeRange
	} from './days';

	interface Props {
		days: string[];
		events: CalendarEvent[];
		colorOf: (event: CalendarEvent) => IntelligenceAgentColor;
		/** Whether `event` can be moved: its calendar isn't read-only. */
		canEdit: (event: CalendarEvent) => boolean;
		onopen: (event: CalendarEvent) => void;
		oncreate: (timing: EventTiming) => void;
		onmove: (event: CalendarEvent, timing: EventTiming) => void;
		/** Shows `day` alone, from its heading. */
		onday: (day: string) => void;
	}

	let { days, events, colorOf, canEdit, onopen, oncreate, onmove, onday }: Props = $props();

	/** One hour's height, in pixels. */
	const HOUR = 48;
	/** The hour labels' column, in pixels. */
	const GUTTER = 56;
	/** An all-day bar's row, in pixels. */
	const BAR = 22;

	const today = $derived(localDay());
	const columns = $derived(`${GUTTER}px repeat(${days.length}, minmax(0, 1fr))`);
	const bars = $derived(layoutBars(events, days));
	const barRows = $derived(Math.max(1, ...bars.map((b) => b.row + 1)));

	let scroller = $state<HTMLElement>();
	let body = $state<HTMLElement>();
	let allDayRow = $state<HTMLElement>();

	onMount(() => {
		if (scroller) scroller.scrollTop = 7.5 * HOUR;
	});

	// The clock, for the line at the time now; it moves each minute.
	let now = $state(Date.now() / 1000);
	onMount(() => {
		const timer = setInterval(() => (now = Date.now() / 1000), 60_000);
		return () => clearInterval(timer);
	});

	// ---- Pointer ------------------------------------------------------------

	/** The column and minute under a pointer over the hours, or the column alone over the all-day row. */
	function at(element: HTMLElement, x: number, y: number) {
		const rect = element.getBoundingClientRect();
		const width = (rect.width - GUTTER) / days.length;
		const column = Math.min(
			days.length - 1,
			Math.max(0, Math.floor((x - rect.left - GUTTER) / width))
		);
		const minute = Math.min(1440, Math.max(0, ((y - rect.top) / HOUR) * 60));
		return { column, minute };
	}

	type Drag =
		| { kind: 'create'; column: number; from: number; to: number }
		| {
				kind: 'move';
				event: CalendarEvent;
				/** How far into the event the pointer took hold, in minutes. */
				grab: number;
				x: number;
				y: number;
				moved: boolean;
				column: number;
				minute: number;
		  }
		| {
				kind: 'moveBar';
				event: CalendarEvent;
				startColumn: number;
				x: number;
				moved: boolean;
				shift: number;
		  };

	let drag = $state<Drag | null>(null);
	/** Set as a drag ends, so a click it ends with doesn't open the event too. */
	let dragged = false;
	function swallowClick() {
		dragged = true;
		setTimeout(() => (dragged = false));
	}

	function startCreate(e: PointerEvent) {
		if (e.button !== 0 || !body) return;
		const { column, minute } = at(body, e.clientX, e.clientY);
		const from = Math.floor(minute / 15) * 15;
		drag = { kind: 'create', column, from, to: from };
		body.setPointerCapture(e.pointerId);
		e.preventDefault();
	}

	function startMove(e: PointerEvent, event: CalendarEvent) {
		if (e.button !== 0 || !body || !canEdit(event) || event.timing.kind !== 'timed') return;
		e.stopPropagation();
		const { column, minute } = at(body, e.clientX, e.clientY);
		const start = minuteOf(event.timing.starts_at);
		drag = {
			kind: 'move',
			event,
			grab: minute - start,
			x: e.clientX,
			y: e.clientY,
			moved: false,
			column,
			minute: start
		};
	}

	function startBar(e: PointerEvent, event: CalendarEvent) {
		if (e.button !== 0 || !allDayRow || !canEdit(event)) return;
		e.stopPropagation();
		const { column } = at(allDayRow, e.clientX, 0);
		drag = { kind: 'moveBar', event, startColumn: column, x: e.clientX, moved: false, shift: 0 };
	}

	/**
	 * Takes the pointer once a press on an event becomes a drag, so a press that
	 * doesn't move stays the event's own click.
	 */
	function capture(e: PointerEvent) {
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	}

	function onmovepointer(e: PointerEvent) {
		if (!drag) return;
		if (drag.kind === 'create' && body) {
			drag.to = snap(at(body, e.clientX, e.clientY).minute);
		} else if (drag.kind === 'move' && body) {
			if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 4) return;
			if (!drag.moved) capture(e);
			drag.moved = true;
			const { column, minute } = at(body, e.clientX, e.clientY);
			const t = drag.event.timing as Extract<EventTiming, { kind: 'timed' }>;
			const length = Math.round((t.ends_at - t.starts_at) / 60);
			drag.column = column;
			drag.minute = Math.min(1440 - Math.min(length, 15), Math.max(0, snap(minute - drag.grab)));
		} else if (drag.kind === 'moveBar' && allDayRow) {
			if (!drag.moved && Math.abs(e.clientX - drag.x) < 4) return;
			if (!drag.moved) capture(e);
			drag.moved = true;
			drag.shift = at(allDayRow, e.clientX, 0).column - drag.startColumn;
		}
	}

	function onup() {
		const d = drag;
		drag = null;
		if (!d) return;
		if (d.kind === 'create') {
			const [lo, hi] =
				d.from === d.to ? [d.from, d.from + 30] : [Math.min(d.from, d.to), Math.max(d.from, d.to)];
			const day = days[d.column];
			oncreate({
				kind: 'timed',
				starts_at: atMinute(day, lo),
				ends_at: atMinute(day, Math.max(hi, lo + 15)),
				time_zone: localZone()
			});
		} else if (d.kind === 'move' && d.moved && d.event.timing.kind === 'timed') {
			swallowClick();
			const timing = moveTo(d.event.timing, days[d.column], d.minute);
			if (timing.kind === 'timed' && timing.starts_at !== d.event.timing.starts_at)
				onmove(d.event, timing);
		} else if (d.kind === 'moveBar' && d.moved) {
			swallowClick();
			if (d.shift !== 0) onmove(d.event, moveByDays(d.event.timing, d.shift));
		}
	}

	function open(event: CalendarEvent) {
		if (!dragged) onopen(event);
	}

	function createAllDay(e: MouseEvent) {
		if (!allDayRow) return;
		const day = days[at(allDayRow, e.clientX, 0).column];
		oncreate({ kind: 'allDay', start_date: day, end_date: day });
	}

	/** Where an event is drawn: while it is dragged, where the drag has it. */
	function placedOf(event: CalendarEvent, column: number, start: number, end: number) {
		if (drag?.kind === 'move' && drag.moved && drag.event.id === event.id)
			return { column: drag.column, start: drag.minute, end: drag.minute + (end - start) };
		return { column, start, end };
	}

	const placed = $derived(
		days.flatMap((day, column) => layoutDay(events, day).map((p) => ({ ...p, column })))
	);

	const nowMinute = $derived(minuteOf(now));
	const nowColumn = $derived(days.indexOf(today));

	const dayName = (day: string) =>
		new Date(dayStart(day) * 1000).toLocaleDateString(undefined, { weekday: 'short' });
	const dayNumber = (day: string) => Number(day.slice(8));
</script>

<div class="flex min-h-0 flex-1 flex-col select-none">
	<!-- The days' headings. -->
	<div class="grid flex-none border-b border-border" style:grid-template-columns={columns}>
		<div></div>
		{#each days as day (day)}
			<button
				class="flex cursor-pointer items-baseline justify-center gap-1.5 py-2 text-sm text-fg-muted hover:text-fg"
				type="button"
				aria-label="Show {new Date(dayStart(day) * 1000).toLocaleDateString(undefined, {
					weekday: 'long',
					month: 'long',
					day: 'numeric'
				})}"
				onclick={() => onday(day)}
			>
				<span class="text-xs">{dayName(day)}</span>
				<span
					class={[
						'grid size-6 place-items-center rounded-full tabular-nums',
						day === today ? 'bg-accent font-semibold text-accent-fg' : 'text-fg'
					]}>{dayNumber(day)}</span
				>
			</button>
		{/each}
	</div>

	<!-- The all-day row: bars over the days, a click on empty room making an all-day event. -->
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div
		bind:this={allDayRow}
		class="relative flex-none border-b border-border"
		style:height="{barRows * BAR + 8}px"
		onclick={createAllDay}
		onpointermove={onmovepointer}
		onpointerup={onup}
		onpointercancel={() => (drag = null)}
	>
		<span
			class="absolute top-1 left-0 pr-2 text-right text-2xs text-fg-subtle"
			style:width="{GUTTER}px">All day</span
		>
		{#each bars as bar (bar.event.id)}
			{@const shift = drag?.kind === 'moveBar' && drag.event.id === bar.event.id ? drag.shift : 0}
			{@const from = Math.max(0, bar.from + shift)}
			{@const span = Math.min(days.length - from, bar.span)}
			{@const color = colorOf(bar.event)}
			<button
				class={[
					'absolute flex cursor-pointer items-center gap-1 truncate rounded-sm px-1.5 text-left text-xs font-medium text-fg',
					agentWash[color]
				]}
				style:top="{4 + bar.row * BAR}px"
				style:height="{BAR - 2}px"
				style:left="calc({GUTTER}px + {from} * (100% - {GUTTER}px) / {days.length} + 2px)"
				style:width="calc({span} * (100% - {GUTTER}px) / {days.length} - 4px)"
				type="button"
				onpointerdown={(e) => startBar(e, bar.event)}
				onclick={(e) => {
					e.stopPropagation();
					open(bar.event);
				}}
			>
				<span class={['h-3 w-0.75 flex-none rounded-full', agentFill[color]]} aria-hidden="true"
				></span>
				<span class="truncate">{bar.event.title}</span>
				{#if bar.event.timing.kind === 'timed'}
					<span class="sr-only">, {timeRange(bar.event.timing)}</span>
				{/if}
			</button>
		{/each}
	</div>

	<!-- The hours. -->
	<div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto">
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			bind:this={body}
			class="relative"
			style:height="{24 * HOUR}px"
			onpointerdown={startCreate}
			onpointermove={onmovepointer}
			onpointerup={onup}
			onpointercancel={() => (drag = null)}
		>
			{#each { length: 24 } as _, hour (hour)}
				<div
					class="pointer-events-none absolute right-0 border-t border-border"
					style:left="{GUTTER}px"
					style:top="{hour * HOUR}px"
				></div>
				{#if hour > 0}
					<span
						class="pointer-events-none absolute left-0 -translate-y-1/2 pr-2 text-right text-2xs text-fg-subtle"
						style:width="{GUTTER}px"
						style:top="{hour * HOUR}px">{hourLabel(hour)}</span
					>
				{/if}
			{/each}
			{#each days as day, column (day)}
				<div
					class="pointer-events-none absolute inset-y-0 border-l border-border"
					style:left="calc({GUTTER}px + {column} * (100% - {GUTTER}px) / {days.length})"
					style:width="calc((100% - {GUTTER}px) / {days.length})"
				></div>
			{/each}

			{#if nowColumn !== -1}
				<div
					class="pointer-events-none absolute z-10 h-0.5 bg-accent"
					style:top="{(nowMinute / 60) * HOUR}px"
					style:left="calc({GUTTER}px + {nowColumn} * (100% - {GUTTER}px) / {days.length})"
					style:width="calc((100% - {GUTTER}px) / {days.length})"
					aria-hidden="true"
				></div>
			{/if}

			{#each placed as p (`${p.event.id}:${p.column}`)}
				{@const where = placedOf(p.event, p.column, p.start, p.end)}
				{@const color = colorOf(p.event)}
				{@const moving = drag?.kind === 'move' && drag.moved && drag.event.id === p.event.id}
				{@const lane = moving ? 0 : p.lane}
				{@const lanes = moving ? 1 : p.lanes}
				<button
					class={[
						'absolute flex cursor-pointer flex-col overflow-hidden rounded-sm border-l-3 px-1.5 py-0.5 text-left text-xs leading-tight text-fg',
						agentWash[color],
						moving && 'z-20 shadow-(--shadow-floating)'
					]}
					style:border-left-color="var(--agent-{color})"
					style:top="{(where.start / 60) * HOUR + 1}px"
					style:height="{Math.max(((where.end - where.start) / 60) * HOUR - 2, 18)}px"
					style:left="calc({GUTTER}px + ({where.column} + {lane} / {lanes}) * (100% - {GUTTER}px) / {days.length}
					+ 1px)"
					style:width="calc((100% - {GUTTER}px) / {days.length} / {lanes} - 3px)"
					type="button"
					onpointerdown={(e) => startMove(e, p.event)}
					onclick={() => open(p.event)}
				>
					<span class="truncate font-medium">{p.event.title}</span>
					{#if where.end - where.start >= 45 && p.event.timing.kind === 'timed'}
						<span class="truncate text-fg-muted">{timeRange(p.event.timing)}</span>
					{:else if p.event.timing.kind === 'timed'}
						<span class="sr-only">, {timeRange(p.event.timing)}</span>
					{/if}
					{#if where.end - where.start >= 75 && p.event.location}
						<span class="truncate text-fg-muted">{p.event.location}</span>
					{/if}
				</button>
			{/each}

			{#if drag?.kind === 'create'}
				{@const lo = drag.from === drag.to ? drag.from : Math.min(drag.from, drag.to)}
				{@const hi = drag.from === drag.to ? drag.from + 30 : Math.max(drag.from, drag.to)}
				<div
					class="pointer-events-none absolute rounded-sm border border-accent bg-accent-soft"
					style:top="{(lo / 60) * HOUR}px"
					style:height="{((hi - lo) / 60) * HOUR}px"
					style:left="calc({GUTTER}px + {drag.column} * (100% - {GUTTER}px) / {days.length} + 1px)"
					style:width="calc((100% - {GUTTER}px) / {days.length} - 3px)"
					aria-hidden="true"
				></div>
			{/if}
		</div>
	</div>
</div>
