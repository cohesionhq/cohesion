// Contacts: the people a workspace keeps, each with the email addresses and
// phone numbers that reach them.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	Contact,
	ContactMethod,
	ContactMethodKind,
	ImageUpload,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

/** A contact's names and notes, as a form holds them; an empty middle name is none. */
export interface ContactFields {
	first_name: string;
	middle_name: string;
	last_name: string;
	notes: string;
}

/** A contact's names in reading order: first, middle, last. */
export function fullName(contact: Contact): string {
	return [contact.first_name, contact.middle_name, contact.last_name].filter(Boolean).join(' ');
}

// As the daemon lists them: by last name, then first name, ignoring case.
const byName = (a: Contact, b: Contact) =>
	a.last_name.localeCompare(b.last_name, undefined, { sensitivity: 'base' }) ||
	a.first_name.localeCompare(b.first_name, undefined, { sensitivity: 'base' }) ||
	a.id - b.id;

export class ContactsStore {
	/** The workspace's live contacts, by last name. */
	readonly list: Resource<Contact[], number>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (workspace_id) => send('listContacts', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'contacts' && key.id === workspace_id
		});
	}

	load(workspace_id: number): Promise<void> {
		return this.list.load(workspace_id);
	}

	#put(contact: Contact) {
		this.list.update((list) =>
			[...list.filter((c) => c.id !== contact.id), contact].toSorted(byName)
		);
	}

	#putMethod(method: ContactMethod, change: (methods: ContactMethod[]) => ContactMethod[]) {
		this.list.update((list) =>
			list.map((c) => (c.id === method.contact_id ? { ...c, methods: change(c.methods) } : c))
		);
	}

	async create(
		workspace_id: number,
		fields: ContactFields,
		methods: { kind: ContactMethodKind; value: string }[]
	): Promise<Contact> {
		const contact = await this.#send('createContact', { workspace_id, ...fields, methods });
		this.#put(contact);
		return contact;
	}

	async update(workspace_id: number, contact_id: number, fields: ContactFields): Promise<Contact> {
		const contact = await this.#send('updateContact', { workspace_id, contact_id, ...fields });
		this.#put(contact);
		return contact;
	}

	async remove(workspace_id: number, contact_id: number): Promise<Contact> {
		const contact = await this.#send('deleteContact', { workspace_id, contact_id });
		this.list.update((list) => list.filter((c) => c.id !== contact_id));
		return contact;
	}

	/** Sets the contact's photo, replacing any it had, or removes it when `photo` is null. */
	async setPhoto(
		workspace_id: number,
		contact_id: number,
		photo: ImageUpload | null
	): Promise<Contact> {
		const contact = await this.#send('setContactPhoto', { workspace_id, contact_id, photo });
		this.#put(contact);
		return contact;
	}

	async addMethod(
		workspace_id: number,
		contact_id: number,
		kind: ContactMethodKind,
		value: string
	): Promise<ContactMethod> {
		const method = await this.#send('addContactMethod', { workspace_id, contact_id, kind, value });
		this.#putMethod(method, (methods) => [...methods, method]);
		return method;
	}

	async updateMethod(
		workspace_id: number,
		method_id: number,
		kind: ContactMethodKind,
		value: string
	): Promise<ContactMethod> {
		const method = await this.#send('updateContactMethod', {
			workspace_id,
			method_id,
			kind,
			value
		});
		this.#putMethod(method, (methods) => methods.map((m) => (m.id === method_id ? method : m)));
		return method;
	}

	async removeMethod(workspace_id: number, method_id: number): Promise<ContactMethod> {
		const method = await this.#send('deleteContactMethod', { workspace_id, method_id });
		this.#putMethod(method, (methods) => methods.filter((m) => m.id !== method_id));
		return method;
	}
}

export function createContactsStore(send: Send = defaultSend): ContactsStore {
	return new ContactsStore(send);
}
