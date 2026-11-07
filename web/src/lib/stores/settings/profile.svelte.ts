// Settings → Profile: the user's own name, bio and photo. It refetches on the
// `profile` key; the profile is read only when the section opens, never with
// anything else, so the photo isn't sent where nothing shows it.

import { send as defaultSend } from '$lib/protocol/api';
import type { ImageUpload, Profile, Send } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

export class ProfileStore {
	readonly profile: Resource<Profile | null>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.profile = new Resource({
			fetch: () => send('getProfile'),
			initial: null,
			matches: (key) => key.kind === 'profile'
		});
	}

	load(): Promise<void> {
		return this.profile.load();
	}

	release(): void {
		this.profile.release();
	}

	/** Sets the user's name, or removes it when `name` is empty. */
	async setName(name: string): Promise<Profile> {
		const updated = await this.#send('setProfileName', { name });
		this.profile.update(() => updated);
		return updated;
	}

	/** Sets the user's bio, or removes it when `bio` is empty. */
	async setBio(bio: string): Promise<Profile> {
		const updated = await this.#send('setProfileBio', { bio });
		this.profile.update(() => updated);
		return updated;
	}

	/** Sets the user's photo, replacing any they had, or removes it when `photo` is null. */
	async setPhoto(photo: ImageUpload | null): Promise<Profile> {
		const updated = await this.#send('setProfilePhoto', { photo });
		this.profile.update(() => updated);
		return updated;
	}
}

export function createProfileStore(send: Send = defaultSend): ProfileStore {
	return new ProfileStore(send);
}

export const profile = createProfileStore();
