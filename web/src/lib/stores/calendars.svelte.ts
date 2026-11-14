// Calendars: the stores that hold them (shown as accounts: Cohesion's own, or
// one per connector account of a calendar backend), the calendars every
// workspace shares, which of them a workspace shows (its perspective), and the
// events of the days a calendar page shows. Stores, calendars and the current
// workspace's perspective are app-wide (the shell's column lists them); each
// half of the sheet has its own events, for the range it shows.

import type {
	Calendar,
	CalendarColorChoice,
	CalendarEvent,
	CalendarEventFields,
	CalendarPerspective,
	CalendarStore,
	CalendarSyncSummary,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export class CalendarsStore {
	readonly stores: Resource<CalendarStore[]>;
	readonly calendars: Resource<Calendar[]>;
	/** Which calendars a workspace shows, keyed by the workspace's id. */
	readonly perspective: Resource<CalendarPerspective | null, number>;

	/** Whether a sync this client asked for is running. */
	syncing = $state(false);

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.stores = new Resource({
			fetch: () => send('listCalendarStores'),
			initial: [],
			matches: (key) => key.kind === 'calendars'
		});
		this.calendars = new Resource({
			fetch: () => send('listCalendars'),
			initial: [],
			matches: (key) => key.kind === 'calendars'
		});
		this.perspective = new Resource({
			fetch: (workspace_id) => send('getCalendarPerspective', { workspace_id }),
			initial: null,
			matches: (key, workspace_id) =>
				key.kind === 'calendars' || (key.kind === 'calendarPerspective' && key.id === workspace_id)
		});
	}

	/** Loads the stores and calendars, and workspace `workspace_id`'s perspective. */
	load(workspace_id: number): Promise<unknown> {
		return Promise.all([
			this.stores.load(),
			this.calendars.load(),
			this.perspective.load(workspace_id)
		]);
	}

	/**
	 * Whether the loaded workspace shows calendar `calendar_id`: every calendar
	 * until it chooses, then the ones it chose.
	 */
	inPerspective(calendar_id: number): boolean {
		const ids = this.perspective.value?.calendar_ids;
		return ids == null || ids.includes(calendar_id);
	}

	/**
	 * Shows or hides calendar `calendar_id` in workspace `workspace_id`: one
	 * that hadn't chosen starts from every calendar.
	 */
	async setInPerspective(workspace_id: number, calendar_id: number, shown: boolean): Promise<void> {
		const loaded = this.perspective.key === workspace_id ? this.perspective.value : null;
		const current = loaded?.calendar_ids ?? this.calendars.value.map((calendar) => calendar.id);
		const calendar_ids = shown
			? [...current.filter((id) => id !== calendar_id), calendar_id]
			: current.filter((id) => id !== calendar_id);
		const before = this.perspective.value;
		if (this.perspective.key === workspace_id) this.perspective.set({ workspace_id, calendar_ids });
		try {
			const saved = await this.#send('setCalendarPerspective', { workspace_id, calendar_ids });
			if (this.perspective.key === workspace_id) this.perspective.set(saved);
		} catch (error) {
			if (this.perspective.key === workspace_id) this.perspective.set(before);
			throw error;
		}
	}

	/** Calendar `id`, if it is loaded. */
	calendar(id: number): Calendar | undefined {
		return this.calendars.value.find((c) => c.id === id);
	}

	/** Store `id`, if it is loaded. */
	store(id: number): CalendarStore | undefined {
		return this.stores.value.find((s) => s.id === id);
	}

	async createStore(title: string): Promise<CalendarStore> {
		const store = await this.#send('createCalendarStore', { title });
		this.stores.update((list) => [...list, store]);
		return store;
	}

	/** Names store `store_id` `title`, or with `null` gives it its backend's name back. */
	async renameStore(store_id: number, title: string | null): Promise<CalendarStore> {
		const store = await this.#send('renameCalendarStore', { store_id, title });
		this.stores.update((list) => list.map((s) => (s.id === store.id ? store : s)));
		return store;
	}

	/** Deletes a local store with its calendars and their events. */
	async deleteStore(store_id: number): Promise<void> {
		await this.#send('deleteCalendarStore', { store_id });
		this.stores.update((list) => list.filter((s) => s.id !== store_id));
		this.calendars.update((list) => list.filter((c) => c.store_id !== store_id));
	}

	/** Makes a calendar from workspace `workspace_id`, whose perspective it joins if it has one. */
	async create(
		store_id: number,
		title: string,
		color: CalendarColorChoice,
		workspace_id: number | null = null
	): Promise<Calendar> {
		const calendar = await this.#send('createCalendar', { store_id, title, color, workspace_id });
		this.calendars.update((list) => [...list, calendar]);
		const loaded = this.perspective.value;
		if (loaded?.calendar_ids && loaded.workspace_id === workspace_id)
			this.perspective.set({ ...loaded, calendar_ids: [...loaded.calendar_ids, calendar.id] });
		return calendar;
	}

	async rename(calendar_id: number, title: string): Promise<Calendar> {
		return this.#replace(await this.#send('renameCalendar', { calendar_id, title }));
	}

	async setColor(calendar_id: number, color: CalendarColorChoice): Promise<Calendar> {
		return this.#replace(await this.#send('setCalendarColor', { calendar_id, color }));
	}

	/** Deletes a local calendar with its events. */
	async remove(calendar_id: number): Promise<void> {
		await this.#send('deleteCalendar', { calendar_id });
		this.calendars.update((list) => list.filter((c) => c.id !== calendar_id));
	}

	/** Syncs every backend account now; the daemon's invalidations bring what changed. */
	async sync(): Promise<CalendarSyncSummary> {
		this.syncing = true;
		try {
			return await this.#send('syncCalendars');
		} finally {
			this.syncing = false;
		}
	}

	#replace(calendar: Calendar): Calendar {
		this.calendars.update((list) => list.map((c) => (c.id === calendar.id ? calendar : c)));
		return calendar;
	}
}

export function createCalendarsStore(send: Send = defaultSend): CalendarsStore {
	return new CalendarsStore(send);
}

export const calendars = createCalendarsStore();

/** A range of time a page shows, in Unix seconds, `from` up to `to`. */
export interface EventRange {
	from: number;
	to: number;
}

const rangeKey = ({ from, to }: EventRange) => `${from}:${to}`;

function parseRange(key: string): EventRange {
	const [from, to] = key.split(':').map(Number);
	return { from, to };
}

/** The events of every calendar over the range one calendar page shows. */
export class CalendarEventsStore {
	/** Keyed by `"from:to"`, so the same range is the same key. */
	readonly list: Resource<CalendarEvent[], string>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (key) => send('listCalendarEvents', { calendar_ids: null, ...parseRange(key) }),
			initial: [],
			matches: (key) => key.kind === 'calendarEvents' || key.kind === 'calendars',
			// Another range keeps the events shown until its own land; each is drawn by its days.
			keepsValue: () => true
		});
	}

	load(range: EventRange): Promise<void> {
		return this.list.load(rangeKey(range));
	}

	/** Event `event_id`, wherever it falls. */
	get(event_id: number): Promise<CalendarEvent> {
		return this.#send('getCalendarEvent', { event_id });
	}

	async create(calendar_id: number, event: CalendarEventFields): Promise<CalendarEvent> {
		const created = await this.#send('createCalendarEvent', { calendar_id, event });
		this.list.update((list) => [...list, created]);
		return created;
	}

	/** Replaces what event `event_id` says, showing it before the daemon confirms. */
	async update(event_id: number, event: CalendarEventFields): Promise<CalendarEvent> {
		const before = this.list.value;
		this.list.update((list) =>
			list.map((e) =>
				e.id === event_id
					? { ...e, ...event, selected_color: e.selected_color, timing: event.timing }
					: e
			)
		);
		try {
			const updated = await this.#send('updateCalendarEvent', { event_id, event });
			this.list.update((list) => list.map((e) => (e.id === updated.id ? updated : e)));
			return updated;
		} catch (error) {
			this.list.set(before);
			throw error;
		}
	}

	async remove(event_id: number): Promise<void> {
		await this.#send('deleteCalendarEvent', { event_id });
		this.list.update((list) => list.filter((e) => e.id !== event_id));
	}
}

export function createCalendarEventsStore(send: Send = defaultSend): CalendarEventsStore {
	return new CalendarEventsStore(send);
}

/** What `event` says, to send back changed. Its selected color goes back as it is. */
export function fieldsOf(event: CalendarEvent): CalendarEventFields {
	return {
		title: event.title,
		location: event.location,
		description: event.description,
		timing: event.timing,
		invitees: event.invitees,
		selected_color: event.selected_color ? { kind: 'palette', color: event.selected_color } : null
	};
}
