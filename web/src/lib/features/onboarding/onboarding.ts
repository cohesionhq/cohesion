// The pages of the first-run flow, shown while there are no workspaces: three
// steps toward a setup that works, each skippable, then a page saying where
// things stand.

export const STEPS = ['workspace', 'intelligence', 'agent'] as const;
export type Step = (typeof STEPS)[number];
export type Page = Step | 'done';

export const PAGES: readonly Page[] = [...STEPS, 'done'];

/** The page after `page`, or `page` itself on the last one. */
export function nextPage(page: Page): Page {
	return PAGES[Math.min(PAGES.indexOf(page) + 1, PAGES.length - 1)];
}

/** The page before `page`, or null on the first one. */
export function previousPage(page: Page): Page | null {
	const index = PAGES.indexOf(page);
	return index > 0 ? PAGES[index - 1] : null;
}

/** How far along the setup is: the share of steps done, from 0 to 1. */
export function readiness(done: ReadonlySet<Step>): number {
	return STEPS.filter((step) => done.has(step)).length / STEPS.length;
}
