// Contacts and the ways to reach them.

import type { MockContact } from '../state';
import { nowSeconds } from '../support';
import type { Handlers, MockCore } from '../core';

export function contactsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		workspace,
		contactRow,
		withMethods,
		contactMethod,
		contactNames,
		requireMethodValue,
		touchContact,
		addMethod,
		methodReply
	} = core;
	return {
		listContacts: ({ workspace_id }) => {
			workspace(workspace_id);
			const name = (c: MockContact) => [c.last_name.toLowerCase(), c.first_name.toLowerCase()];
			return state.contacts
				.filter((c) => c.workspace_id === workspace_id && !c.deleted)
				.sort((a, b) => {
					const [al, af] = name(a);
					const [bl, bf] = name(b);
					return al.localeCompare(bl) || af.localeCompare(bf) || a.id - b.id;
				})
				.map(withMethods);
		},
		getContact: ({ workspace_id, contact_id }) => withMethods(contactRow(workspace_id, contact_id)),
		createContact: ({ workspace_id, first_name, middle_name, last_name, notes, methods }) => {
			workspace(workspace_id);
			const names = contactNames(first_name, middle_name, last_name);
			methods.forEach((m) => requireMethodValue(m.value));
			const now = nowSeconds();
			const created: MockContact = {
				id: nextId(state.contacts),
				workspace_id,
				deleted: false,
				...names,
				notes,
				created_at: now,
				modified: now,
				photo: { kind: 'none' }
			};
			state.contacts.push(created);
			methods.forEach((m) => addMethod(created.id, m.kind, m.value));
			return withMethods(created);
		},
		updateContact: ({ workspace_id, contact_id, first_name, middle_name, last_name, notes }) => {
			const names = contactNames(first_name, middle_name, last_name);
			const c = contactRow(workspace_id, contact_id);
			Object.assign(c, names, { notes, modified: nowSeconds() });
			return withMethods(c);
		},
		deleteContact: ({ workspace_id, contact_id }) => {
			const c = contactRow(workspace_id, contact_id);
			c.deleted = true;
			for (const t of state.tasks)
				if (t.assignee?.kind === 'contact' && t.assignee.contact_id === contact_id)
					t.assignee = null;
			return withMethods(c);
		},
		setContactPhoto: ({ workspace_id, contact_id, photo }) => {
			const c = contactRow(workspace_id, contact_id);
			c.photo = photo
				? { kind: 'image', photo_media_type: photo.media_type, photo_data: photo.data }
				: { kind: 'none' };
			c.modified = nowSeconds();
			return withMethods(c);
		},
		addContactMethod: ({ workspace_id, contact_id, kind, value }) => {
			contactRow(workspace_id, contact_id);
			return methodReply(addMethod(contact_id, kind, value));
		},
		updateContactMethod: ({ workspace_id, method_id, kind, value }) => {
			requireMethodValue(value);
			const m = contactMethod(workspace_id, method_id);
			Object.assign(m, { kind, value, modified: nowSeconds() });
			touchContact(m.contact_id);
			return methodReply(m);
		},
		deleteContactMethod: ({ workspace_id, method_id }) => {
			const m = contactMethod(workspace_id, method_id);
			state.contactMethods = state.contactMethods.filter((other) => other !== m);
			touchContact(m.contact_id);
			return methodReply(m);
		}
	} satisfies Partial<Handlers>;
}
