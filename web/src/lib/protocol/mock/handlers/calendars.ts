// Calendars: their stores, the calendars and their events. Every store in
// the mock is local, so nothing syncs.

import { ProtocolError } from '../../errors';
import type {
	CalendarColor,
	CalendarColorChoice,
	CalendarEvent,
	CalendarEventFields,
	IntelligenceAgentColor
} from '../../types';
import { nowSeconds } from '../support';
import type { Handlers, MockCore } from '../core';

/** The palette's exact values, as the daemon's `IntelligenceAgentColor::hex` has them. */
const PALETTE: Record<IntelligenceAgentColor, string> = {
	blue: '#4a6fb5',
	green: '#4f8a5a',
	orange: '#c4702e',
	purple: '#7a5bb0',
	red: '#b54848',
	pink: '#b8578f',
	teal: '#3d8a8a',
	yellow: '#b8932a'
};

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

/** `choice` as an exact color; the nearest palette color by plain RGB distance, near enough for the mock. */
export function mockColor(choice: CalendarColorChoice): CalendarColor {
	if (choice.kind === 'palette') return { hex: PALETTE[choice.color], nearest: choice.color };
	const hex = choice.hex.toLowerCase();
	if (!/^#[0-9a-f]{6}$/.test(hex))
		throw new ProtocolError('validationFailed', 'A color must be written as #rrggbb.', 'color');
	const [r, g, b] = rgb(hex);
	const distance = (color: IntelligenceAgentColor) => {
		const [pr, pg, pb] = rgb(PALETTE[color]);
		return (r - pr) ** 2 + (g - pg) ** 2 + (b - pb) ** 2;
	};
	const colors = Object.keys(PALETTE) as IntelligenceAgentColor[];
	const nearest = colors.reduce((best, color) => (distance(color) < distance(best) ? color : best));
	return { hex, nearest };
}

/** Where an event starts, in Unix seconds, to order and filter by. */
function startOf(event: CalendarEvent): number {
	return event.timing.kind === 'timed'
		? event.timing.starts_at
		: Date.parse(`${event.timing.start_date}T00:00:00Z`) / 1000;
}

function endOf(event: CalendarEvent): number {
	return event.timing.kind === 'timed'
		? event.timing.ends_at
		: Date.parse(`${event.timing.end_date}T00:00:00Z`) / 1000 + 86_400;
}

export function calendarsHandlers(core: MockCore) {
	const { state, nextId, notFound } = core;
	const requireTitle = (title: string) => {
		if (title === '') throw new ProtocolError('validationFailed', 'Title can’t be empty.');
	};
	const store = (id: number) => state.calendarStores.find((s) => s.id === id) ?? notFound('store');
	const calendar = (id: number) =>
		state.calendars.find((c) => c.id === id && !c.deleted) ?? notFound('calendar');
	const event = (id: number) =>
		state.calendarEvents.find((e) => e.id === id && !e.deleted) ?? notFound('calendar event');
	const fields = (event: CalendarEventFields) => {
		requireTitle(event.title);
		return {
			title: event.title,
			location: event.location,
			description: event.description,
			timing: event.timing,
			invitees: event.invitees,
			color: event.color ? mockColor(event.color) : null
		};
	};
	return {
		listCalendarStores: () => [...state.calendarStores],
		createCalendarStore: ({ title }) => {
			requireTitle(title);
			const created = {
				id: nextId(state.calendarStores),
				title,
				backend: { kind: 'local' as const },
				last_synced_at: null,
				sync_failure: null,
				created_at: nowSeconds()
			};
			state.calendarStores.push(created);
			return created;
		},
		deleteCalendarStore: ({ store_id }) => {
			const s = store(store_id);
			state.calendarStores = state.calendarStores.filter((other) => other !== s);
			const gone = state.calendars.filter((c) => c.store_id === s.id).map((c) => c.id);
			state.calendars = state.calendars.filter((c) => c.store_id !== s.id);
			state.calendarEvents = state.calendarEvents.filter((e) => !gone.includes(e.calendar_id));
			return s;
		},
		listCalendars: () =>
			state.calendars
				.filter((c) => !c.deleted)
				.sort((a, b) => a.store_id - b.store_id || a.title.localeCompare(b.title)),
		createCalendar: ({ store_id, title, color }) => {
			requireTitle(title);
			store(store_id);
			const now = nowSeconds();
			const created = {
				id: nextId(state.calendars),
				store_id,
				deleted: false,
				title,
				color: mockColor(color),
				remote_identifier: null,
				read_only: false,
				created_at: now,
				modified: now
			};
			state.calendars.push(created);
			return created;
		},
		renameCalendar: ({ calendar_id, title }) => {
			requireTitle(title);
			return Object.assign(calendar(calendar_id), { title, modified: nowSeconds() });
		},
		setCalendarColor: ({ calendar_id, color }) =>
			Object.assign(calendar(calendar_id), { color: mockColor(color), modified: nowSeconds() }),
		deleteCalendar: ({ calendar_id }) => {
			const c = Object.assign(calendar(calendar_id), { deleted: true });
			for (const e of state.calendarEvents) if (e.calendar_id === c.id) e.deleted = true;
			return c;
		},
		listCalendarEvents: ({ calendar_ids, from, to }) =>
			state.calendarEvents
				.filter(
					(e) =>
						!e.deleted &&
						(calendar_ids == null || calendar_ids.includes(e.calendar_id)) &&
						startOf(e) < to &&
						endOf(e) > from
				)
				.sort((a, b) => startOf(a) - startOf(b) || a.id - b.id),
		getCalendarEvent: ({ event_id }) => event(event_id),
		createCalendarEvent: ({ calendar_id, event: given }) => {
			calendar(calendar_id);
			const now = nowSeconds();
			const created: CalendarEvent = {
				id: nextId(state.calendarEvents),
				calendar_id,
				deleted: false,
				...fields(given),
				remote_identifier: null,
				remote_modified: null,
				pending_push: false,
				revision: 0,
				created_at: now,
				modified: now
			};
			state.calendarEvents.push(created);
			return created;
		},
		updateCalendarEvent: ({ event_id, event: given }) => {
			const e = event(event_id);
			return Object.assign(e, fields(given), { revision: e.revision + 1, modified: nowSeconds() });
		},
		deleteCalendarEvent: ({ event_id }) => {
			const e = event(event_id);
			return Object.assign(e, { deleted: true, revision: e.revision + 1, modified: nowSeconds() });
		},
		syncCalendars: () => ({ synced: 0, failed: 0, pulled: 0, pushed: 0, conflicts: 0 })
	} satisfies Partial<Handlers>;
}
