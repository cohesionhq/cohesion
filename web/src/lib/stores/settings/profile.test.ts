import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createProfileStore } from './profile.svelte';

describe('profile store', () => {
	it('sets, replaces and removes the photo', async () => {
		const mock = createMock();
		const store = createProfileStore(mock.send);
		await store.load();
		expect(store.profile.value?.photo).toEqual({ kind: 'none' });

		await store.setPhoto({ media_type: 'image/png', data: 'AAAA' });
		await store.setPhoto({ media_type: 'image/jpeg', data: 'BBBB' });
		const image = { kind: 'image', photo_media_type: 'image/jpeg', photo_data: 'BBBB' };
		expect(store.profile.value?.photo).toEqual(image);
		expect(mock.state.profile.photo).toEqual(image);

		await store.setPhoto(null);
		expect(store.profile.value?.photo).toEqual({ kind: 'none' });
	});

	it('sets the name, trimmed, and removes an empty one', async () => {
		const mock = createMock();
		const store = createProfileStore(mock.send);
		await store.load();
		expect(store.profile.value?.name).toBeNull();

		await store.setName('  Ada Lovelace ');
		expect(store.profile.value?.name).toBe('Ada Lovelace');
		await store.setName('');
		expect(store.profile.value?.name).toBeNull();
	});

	it('sets the bio, trimmed, removes an empty one and keeps it when one is refused', async () => {
		const mock = createMock();
		const store = createProfileStore(mock.send);
		await store.load();
		expect(store.profile.value?.bio).toBeNull();

		await store.setBio('  I run a bakery.\nMornings are busy. ');
		expect(store.profile.value?.bio).toBe('I run a bakery.\nMornings are busy.');
		await expect(store.setBio('a'.repeat(1001))).rejects.toThrow();
		expect(store.profile.value?.bio).toBe('I run a bakery.\nMornings are busy.');
		await store.setBio('  ');
		expect(store.profile.value?.bio).toBeNull();
	});
});
