// Where a page's back link goes: the page the user came from, named by its
// title, stored on the history entry so it survives going back and forward.
// A page reached without one (a fresh tab, a reload of the first page) keeps
// the link it was given.

import { afterNavigate, beforeNavigate, replaceState } from '$app/navigation';
import { page } from '$app/state';

export interface Back {
	href: string;
	/** The name of the page it leads back to. */
	label: string;
}

/** The name a page's title gives it: a title is "{name} · {section}". */
export function pageName(title: string): string {
	return title.split(' · ')[0];
}

/** Records the page being left on the page arrived at; call once, from the root layout. */
export function trackBack() {
	let leaving: Back | null = null;

	beforeNavigate(({ from }) => {
		leaving = from && {
			href: from.url.pathname + from.url.search,
			label: pageName(document.title)
		};
	});

	afterNavigate(({ type, from, to }) => {
		// Back and forward keep the entry's own, and a replaced entry took the old one's (`mainPane`).
		if (type === 'popstate' || type === 'enter' || page.state.back !== undefined) return;
		// A query change (a list's view) is the same page.
		if (!leaving || from?.url.pathname === to?.url.pathname) return;
		replaceState('', { back: leaving });
	});
}
