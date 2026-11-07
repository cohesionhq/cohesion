import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createContactsStore, fullName } from './contacts.svelte';

const fields = (first_name: string, last_name: string) => ({
	first_name,
	middle_name: '',
	last_name,
	notes: ''
});

describe('contacts store', () => {
	it('keeps contacts by last name as they are added, renamed and deleted', async () => {
		const store = createContactsStore(createMock().send);
		await store.load(2);
		const zed = await store.create(2, fields('Ada', 'Zed'), []);
		await store.create(2, fields('Bo', 'Abel'), []);
		expect(store.list.value.map((c) => c.last_name)).toEqual(['Abel', 'Zed']);
		await store.update(2, zed.id, { ...fields('Ada', 'Aaron'), middle_name: 'M' });
		expect(store.list.value.map(fullName)).toEqual(['Ada M Aaron', 'Bo Abel']);
		await expect(store.update(2, zed.id, fields('Ada', ''))).rejects.toMatchObject({
			field: 'last_name'
		});
		await store.remove(2, zed.id);
		expect(store.list.value.map(fullName)).toEqual(['Bo Abel']);
	});

	it('sets and removes a contact’s photo in place', async () => {
		const store = createContactsStore(createMock().send);
		await store.load(2);
		const contact = await store.create(2, fields('Ada', 'Lovelace'), []);
		expect(store.list.value[0].photo).toEqual({ kind: 'none' });
		await store.setPhoto(2, contact.id, { media_type: 'image/png', data: 'AAAA' });
		expect(store.list.value[0].photo).toEqual({
			kind: 'image',
			photo_media_type: 'image/png',
			photo_data: 'AAAA'
		});
		await store.setPhoto(2, contact.id, null);
		expect(store.list.value[0].photo).toEqual({ kind: 'none' });
	});

	it('adds, changes and removes a contact’s methods in place', async () => {
		const store = createContactsStore(createMock().send);
		await store.load(2);
		const contact = await store.create(2, fields('Ada', 'Lovelace'), [
			{ kind: 'email', value: 'ada@example.com' }
		]);
		const phone = await store.addMethod(2, contact.id, 'phone', '555 0100');
		const methods = () => store.list.value[0].methods.map((m) => `${m.kind}:${m.value}`);
		expect(methods()).toEqual(['email:ada@example.com', 'phone:555 0100']);
		await store.updateMethod(2, phone.id, 'email', 'ada@work.example');
		expect(methods()).toEqual(['email:ada@example.com', 'email:ada@work.example']);
		await store.removeMethod(2, store.list.value[0].methods[0].id);
		expect(methods()).toEqual(['email:ada@work.example']);
	});
});
