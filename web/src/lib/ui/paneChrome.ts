// What a page's header takes from the half of the sheet it is drawn in: the
// way back and the split view's controls. The shell provides it
// (`shell/PaneHost.svelte`), so the header works in either half without
// knowing about them.

import { getContext, setContext, type Snippet } from 'svelte';
import type { Back } from '$lib/shell/back';

export interface PaneChrome {
	/** The page this half was reached from, when it was reached from one. */
	readonly back: Back | null | undefined;
	/** Steps this half back to it. */
	goBack(): void;
	/** Drawn at the trailing end of the page's header. */
	readonly controls: Snippet | undefined;
	/** Whether the column right of the sheet speaks for this half, from 1100px: the main half's. */
	readonly hasGutter: boolean;
}

const KEY = Symbol('pane-chrome');

export function setPaneChrome(chrome: PaneChrome) {
	setContext(KEY, chrome);
}

export function paneChrome(): PaneChrome | undefined {
	return getContext<PaneChrome | undefined>(KEY);
}
