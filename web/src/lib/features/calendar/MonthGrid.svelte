<!--
	The month view: six weeks of days, each listing its events (all-day ones
	first, then by time), as many as fit and "N more", which shows the day.
	Clicking a day's empty room makes an all-day event that day; an event drags
	to another day, keeping its time, and a click opens it.
-->
<script lang="ts">
	import { localDay } from '$lib/format/dates';
	import { shortTime } from '$lib/format/dates';
	import type { CalendarEvent, EventTiming, IntelligenceAgentColor } from '$lib/protocol/types';
	import { agentFill, agentWash } from '$lib/ui/colors';
	import { dayStart, daysBetween, isOn, moveByDays } from './days';

	interface Props {
		days: string[];
		/** The month shown, as "2026-11"; its days stand out from the weeks' others. */
		month: string;
		events: CalendarEvent[];
		colorOf: (event: CalendarEvent) => IntelligenceAgentColor;
		canEdit: (event: CalendarEvent) => boolean;
		onopen: (event: CalendarEvent) => void;
		oncreate: (timing: EventTiming) => void;
		onmove: (event: CalendarEvent, timing: EventTiming) => void;
		onday: (day: string) => void;
	}

	let { days, month, events, colorOf, canEdit, onopen, oncreate, onmove, onday }: Props = $props();

	/** The most events a day lists before "N more". */
	const FITS = 3;

	const today = $derived(localDay());

	/** The events on `day`, all-day ones first, then by when they start. */
	function eventsOn(day: string): CalendarEvent[] {
		const startOf = (e: CalendarEvent) =>
			e.timing.kind === 'timed' ? e.timing.starts_at : dayStart(e.timing.start_date);
		return events
			.filter((e) => isOn(e, day))
			.sort(
				(a, b) =>
					Number(a.timing.kind === 'timed') - Number(b.timing.kind === 'timed') ||
					startOf(a) - startOf(b)
			);
	}
	const byDay = $derived(new Map(days.map((day) => [day, eventsOn(day)])));

	// ---- Dragging an event to another day ------------------------------------

	let grid = $state<HTMLElement>();
	let drag = $state<{
		event: CalendarEvent;
		from: string;
		to: string;
		x: number;
		y: number;
		moved: boolean;
	} | null>(null);
	let dragged = false;

	function dayAt(x: number, y: number): string | null {
		if (!grid) return null;
		const rect = grid.getBoundingClientRect();
		const column = Math.floor(((x - rect.left) / rect.width) * 7);
		const row = Math.floor(((y - rect.top) / rect.height) * (days.length / 7));
		if (column < 0 || column > 6 || row < 0 || row >= days.length / 7) return null;
		return days[row * 7 + column];
	}

	function start(e: PointerEvent, event: CalendarEvent, day: string) {
		if (e.button !== 0 || !grid || !canEdit(event)) return;
		e.stopPropagation();
		drag = { event, from: day, to: day, x: e.clientX, y: e.clientY, moved: false };
	}

	function move(e: PointerEvent) {
		if (!drag) return;
		if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 4) return;
		// Only a press that moves takes the pointer, so one that doesn't stays the event's click.
		if (!drag.moved) grid?.setPointerCapture(e.pointerId);
		drag.moved = true;
		drag.to = dayAt(e.clientX, e.clientY) ?? drag.to;
	}

	function end() {
		const d = drag;
		drag = null;
		if (!d?.moved) return;
		// A click the drag ends with doesn't open the event too.
		dragged = true;
		setTimeout(() => (dragged = false));
		const shift = daysBetween(d.from, d.to);
		if (shift !== 0) onmove(d.event, moveByDays(d.event.timing, shift));
	}

	function open(event: CalendarEvent) {
		if (!dragged) onopen(event);
	}

	const weekdays = $derived(
		days
			.slice(0, 7)
			.map((day) =>
				new Date(dayStart(day) * 1000).toLocaleDateString(undefined, { weekday: 'short' })
			)
	);
</script>

<div class="flex min-h-0 flex-1 flex-col select-none">
	<div class="grid flex-none grid-cols-7 border-b border-border">
		{#each weekdays as name (name)}
			<span class="py-2 text-center text-xs text-fg-muted">{name}</span>
		{/each}
	</div>
	<div
		bind:this={grid}
		class="grid min-h-0 flex-1 grid-cols-7"
		style:grid-template-rows="repeat({days.length / 7}, minmax(6.5rem, 1fr))"
		onpointermove={move}
		onpointerup={end}
		onpointercancel={() => (drag = null)}
		role="grid"
		tabindex="-1"
	>
		{#each days as day, i (day)}
			{@const list = byDay.get(day) ?? []}
			{@const shown = list.length > FITS ? list.slice(0, FITS - 1) : list}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div
				class={[
					'flex min-w-0 flex-col gap-0.5 border-border p-1',
					i % 7 !== 0 && 'border-l',
					i >= 7 && 'border-t',
					drag?.moved && drag.to === day && 'bg-bg-hover'
				]}
				role="gridcell"
				tabindex="-1"
				onclick={() => oncreate({ kind: 'allDay', start_date: day, end_date: day })}
			>
				<button
					class={[
						'grid size-6 flex-none cursor-pointer place-items-center self-end rounded-full text-xs tabular-nums hover:bg-bg-hover',
						day === today
							? 'bg-accent font-semibold text-accent-fg hover:bg-accent'
							: day.startsWith(month)
								? 'text-fg'
								: 'text-fg-subtle'
					]}
					type="button"
					aria-label="Show {new Date(dayStart(day) * 1000).toLocaleDateString(undefined, {
						weekday: 'long',
						month: 'long',
						day: 'numeric'
					})}"
					onclick={(e) => {
						e.stopPropagation();
						onday(day);
					}}>{Number(day.slice(8))}</button
				>
				{#each shown as event (event.id)}
					{@const color = colorOf(event)}
					{@const timed = event.timing.kind === 'timed'}
					<button
						class={[
							'flex min-w-0 cursor-pointer items-center gap-1 rounded-sm px-1 text-left text-xs leading-5 text-fg',
							!timed && agentWash[color],
							timed && 'hover:bg-bg-hover',
							drag?.moved && drag.event.id === event.id && 'opacity-50'
						]}
						type="button"
						onpointerdown={(e) => start(e, event, day)}
						onclick={(e) => {
							e.stopPropagation();
							open(event);
						}}
					>
						<span
							class={[
								'flex-none',
								timed ? 'size-1.5 rounded-full' : 'h-3 w-0.75 rounded-full',
								agentFill[color]
							]}
							aria-hidden="true"
						></span>
						{#if event.timing.kind === 'timed'}
							<span class="flex-none text-fg-muted tabular-nums"
								>{shortTime(event.timing.starts_at)}</span
							>
						{/if}
						<span class="truncate">{event.title}</span>
					</button>
				{/each}
				{#if list.length > shown.length}
					<button
						class="cursor-pointer self-start rounded-sm px-1 text-xs text-fg-muted hover:bg-bg-hover hover:text-fg"
						type="button"
						onclick={(e) => {
							e.stopPropagation();
							onday(day);
						}}>{list.length - shown.length} more</button
					>
				{/if}
			</div>
		{/each}
	</div>
</div>
