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
// fills in after. Data that outlasts the wait fades in as a second, shorter
// step once the first fade has finished, rather than popping in.
//
// How the fade looks is in tokens.css; a column that slides open or shut
// (`ConversationGutter`) asks `pageFading` and doesn't slide during one.

import { tick } from 'svelte';
import { firstFetchesSettled, type Reveal } from '$lib/stores/resource.svelte';

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

/** The transition running, if any. */
let running: ViewTransition | undefined;
/** Counts page navigations, so a late reveal can tell it's been overtaken by one. */
let navigations = 0;

/** Whether a page fade is running, during which a column opens or shuts at once and fades with the page. */
export function pageFading(): boolean {
	return running !== undefined;
}

/** Starts a transition that cross-fades `target` (named `page` for tokens.css) while `update` runs. */
function crossFade(target: HTMLElement, update: () => Promise<void>): ViewTransition {
	for (const el of document.querySelectorAll<HTMLElement>('[data-view-page], [data-view-query]')) {
		el.style.viewTransitionName = el === target ? 'page' : '';
	}
	const transition = document.startViewTransition(update);
	running = transition;
	transition.ready.catch(() => undefined);
	transition.finished.finally(() => {
		if (running !== transition) return;
		running = undefined;
		target.style.viewTransitionName = '';
	});
	return transition;
}

/**
 * Puts late data on the page in a fade of `target` of its own, once `after`
 * has finished, gathering whatever lands before that fade captures the page.
 * Data for a page a newer navigation has left goes on at once.
 */
function revealer(target: HTMLElement, after: ViewTransition): Reveal {
	const navigation = navigations;
	let previous: Promise<unknown> = after.finished;
	let waiting: (() => void)[] = [];
	let batch: Promise<void> | undefined;
	const showAll = () => {
		const shows = waiting;
		waiting = [];
		batch = undefined;
		for (const show of shows) show();
	};
	return (show) => {
		if (navigation !== navigations) {
			show();
			return Promise.resolve();
		}
		waiting.push(show);
		batch ??= previous
			.catch(() => undefined)
			.then(
				() =>
					new Promise<void>((resolve) => {
						if (navigation !== navigations) {
							showAll();
							resolve();
							return;
						}
						const transition = crossFade(target, async () => {
							showAll();
							resolve();
							await tick();
						});
						previous = transition.finished;
					})
			);
		return batch;
	};
}

/**
 * Cross-fades `target` while `update` swaps what it shows, or just runs
 * `update` where the browser can't. Resolves once `update` has started, so a
 * router can carry on; a second fade skips the first, which rejects its
 * `ready`.
 */
export function fade(target: HTMLElement, update: () => Promise<void>): Promise<void> {
	if (!document.startViewTransition) return update();
	navigations += 1;
	return new Promise((resolve) => {
		const transition: ViewTransition = crossFade(target, async () => {
			resolve();
			await update();
			await tick();
			await firstFetchesSettled(PAGE_DATA_WAIT_MS, revealer(target, transition));
			await tick();
		});
	});
}
