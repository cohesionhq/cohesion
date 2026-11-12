// Where each page was scrolled to, so coming back to it (back, forward, the
// sidebar, a link) puts it where it was. One memory per half's scroller
// (`PaneHost.svelte`), keyed by the page's path and query.

const GIVE_UP_MS = 2000;
const CANCEL_EVENTS = ['wheel', 'touchstart', 'keydown', 'pointerdown'] as const;

export class ScrollMemory {
	#tops = new Map<string, number>();
	#key = '';
	/** Ends the restore in progress, if any. */
	#end: (() => void) | undefined;

	constructor(private readonly scroller: () => HTMLElement | undefined) {}

	/** Records where the user has scrolled the current page to. */
	onscroll() {
		const el = this.scroller();
		// Our own moves while a page is still loading would record a clamped position.
		if (el && !this.#end) this.#tops.set(this.#key, el.scrollTop);
	}

	/**
	 * Moves to `key`'s page: its remembered position, or the top for one not
	 * seen. A page still loading may be too short to scroll that far yet, so
	 * it keeps trying as the page grows, until the user scrolls or it gives up.
	 */
	enter(key: string) {
		if (key === this.#key) return;
		this.#end?.();
		this.#key = key;
		const el = this.scroller();
		if (!el) return;
		const top = this.#tops.get(key) ?? 0;
		el.scrollTop = top;
		if (top === 0 || el.scrollTop >= top - 1) return;

		const observer = new ResizeObserver(() => {
			el.scrollTop = top;
			if (el.scrollTop >= top - 1) end();
		});
		for (const child of el.children) observer.observe(child);
		const timer = setTimeout(() => end(), GIVE_UP_MS);
		const end = () => {
			observer.disconnect();
			clearTimeout(timer);
			for (const name of CANCEL_EVENTS) el.removeEventListener(name, end);
			if (this.#end === end) this.#end = undefined;
		};
		for (const name of CANCEL_EVENTS) el.addEventListener(name, end, { passive: true });
		this.#end = end;
	}

	destroy() {
		this.#end?.();
	}
}
