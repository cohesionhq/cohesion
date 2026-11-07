// How the page words a contact method's kind.

import type { ContactMethodKind } from '$lib/protocol/types';

/** What each kind is called, as a label and as a menu choice. */
export const methodLabels: Record<ContactMethodKind, string> = {
	email: 'Email',
	phone: 'Phone'
};

/** Where a method's value goes when followed: a new email, or a call. */
export function methodHref(kind: ContactMethodKind, value: string): string {
	return kind === 'email' ? `mailto:${value}` : `tel:${value.replace(/[^\d+]/g, '')}`;
}
