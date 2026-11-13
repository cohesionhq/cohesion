import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { atMinute, dayStart } from '$lib/features/calendar/days';
import { localDay } from '$lib/format/dates';
import { createCalendarEventsStore, createCalendarsStore, fieldsOf } from './calendars.svelte';

const week = () => {
	const from = dayStart(localDay()) - 3 * 86_400;
	return { from, to: from + 7 * 86_400 };
};

describe('calendar events store', () => {
	it('moves an event in place, and puts it back when the daemon refuses', async () => {
		const mock = createMock();
		const store = createCalendarEventsStore(mock.send);
		await store.load(week());
		// Copied: the mock's events are its own rows, which a change rewrites.
		const dentist = structuredClone(store.list.value.find((e) => e.title === 'Dentist')!);
		const later = atMinute(localDay(), 14 * 60);
		const moving = store.update(dentist.id, {
			...fieldsOf(dentist),
			timing: { kind: 'timed', starts_at: later, ends_at: later + 3600, time_zone: 'UTC' }
		});
		// Shown before the reply lands.
		expect(store.list.value.find((e) => e.id === dentist.id)?.timing).toMatchObject({
			starts_at: later
		});
		expect((await moving).revision).toBe(dentist.revision + 1);

		await expect(
			store.update(dentist.id, { ...fieldsOf(dentist), title: '' })
		).rejects.toMatchObject({ code: 'validationFailed' });
		expect(store.list.value.find((e) => e.id === dentist.id)?.timing).toMatchObject({
			starts_at: later
		});
	});

	it('keeps an event’s exact color when another field changes', async () => {
		const mock = createMock();
		const store = createCalendarEventsStore(mock.send);
		await store.load(week());
		const climbing = structuredClone(store.list.value.find((e) => e.title === 'Climbing')!);
		const renamed = await store.update(climbing.id, { ...fieldsOf(climbing), title: 'Bouldering' });
		expect(renamed.color).toEqual(climbing.color);
	});
});

describe('calendars store', () => {
	it('drops a deleted store’s calendars', async () => {
		const mock = createMock();
		const store = createCalendarsStore(mock.send);
		await store.load();
		const made = await store.createStore('Family');
		await store.create(made.id, 'Birthdays', { kind: 'palette', color: 'pink' });
		expect(store.calendars.value.some((c) => c.title === 'Birthdays')).toBe(true);
		await store.deleteStore(made.id);
		expect(store.calendars.value.some((c) => c.title === 'Birthdays')).toBe(false);
		expect(store.store(made.id)).toBeUndefined();
	});
});
