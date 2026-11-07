// Pages dissolve into each other where the browser supports view
// transitions; the sidebar stays put. What fades is the deepest
// `data-view-page` element in the half that navigates whose route scope
// holds both ends of the navigation, so moving between settings sections
// fades only the section, not the settings nav. A query-only change swaps in
// place, unless it changes the parameter a `data-view-query` element names:
// then that element fades alone, so picking a view in a list's side nav fades
// the list and not the nav. The new page is captured once its first data has
// landed (or after a short wait), so the fade lands on the page as it will
// look, its side nav and content together, rather than on an empty one that
// fills in after.

import { tick } from 'svelte';
import { firstFetchesSettled } from '$lib/stores/resource.svelte';

const PAGE_DATA_WAIT_MS = 300;

/** One end of a navigation. */
export interface Place {
	url: URL;
	route: { id: string | null };
}

/** What fades when the half drawn in `root` goes from `from` to `to`, if anything does. */
export function fadingScope(
	root: HTMLElement,
	from: Place | null | undefined,
	to: Place | null | undefined
): HTMLElement | undefined {
	const inside = (selector: string) => [
		...(root.matches(selector) ? [root] : []),
		...root.querySelectorAll<HTMLElement>(selector)
	];
	if (from?.url.pathname === to?.url.pathname) {
		return inside('[data-view-query]').findLast(
			(el) =>
				from?.url.searchParams.get(el.dataset.viewQuery!) !==
				to?.url.searchParams.get(el.dataset.viewQuery!)
		);
	}
	const within = (id: string | null | undefined, scope: string) =>
		id === scope || id?.startsWith(scope === '/' ? scope : `${scope}/`);
	return inside('[data-view-page]').findLast(
		(el) =>
			within(from?.route.id, el.dataset.viewPage!) && within(to?.route.id, el.dataset.viewPage!)
	);
}

let fading: ViewTransition | undefined;

/**
 * Cross-fades `target` while `update` swaps what it shows, or just runs
 * `update` where the browser can't. Resolves once `update` has started, so a
 * router can carry on; a second fade skips the first, which rejects its
 * `ready`.
 */
export function fade(target: HTMLElement, update: () => Promise<void>): Promise<void> {
	if (!document.startViewTransition) return update();
	for (const el of document.querySelectorAll<HTMLElement>('[data-view-page], [data-view-query]')) {
		el.style.viewTransitionName = el === target ? 'page' : '';
	}
	return new Promise((resolve) => {
		const transition = document.startViewTransition(async () => {
			resolve();
			await update();
			await tick();
			await firstFetchesSettled(PAGE_DATA_WAIT_MS);
			await tick();
		});
		fading = transition;
		transition.ready.catch(() => undefined);
		transition.finished.finally(() => {
			if (fading === transition) target.style.viewTransitionName = '';
		});
	});
}
