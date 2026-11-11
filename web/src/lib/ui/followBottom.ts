// A feed that opens at its newest message and follows new ones while the
// reader is at (or near) the bottom; scrolled up, it leaves them be. A
// conversation with an agent and an actor session's transcript both scroll
// this way.

/** How close to the bottom, in pixels, still counts as at it. */
const NEAR = 48;

/** What following reads of the element that scrolls. */
interface Scrolled {
	scrollTop: number;
	scrollHeight: number;
	clientHeight: number;
}

export class BottomFollower {
	/** Whether the feed keeps to the bottom as it grows. */
	following = true;
	#lastTop = 0;

	/** Starts over at the bottom, for another conversation. */
	reset(): void {
		this.following = true;
		this.#lastTop = 0;
	}

	/** Keeps to the bottom from now on: sending is a request to see the end. */
	follow(): void {
		this.following = true;
	}

	/**
	 * Takes a scroll of `scroller`. Reaching the bottom follows again; only
	 * scrolling up lets go of it. A scroll event lands after the frame that
	 * caused it, so by then the feed may have grown past a jump to the bottom
	 * (the history mounting, a reply rendering); judged on distance alone,
	 * that would read as the reader leaving and strand the feed partway up.
	 */
	scrolled(scroller: Scrolled): void {
		const top = scroller.scrollTop;
		if (scroller.scrollHeight - top - scroller.clientHeight < NEAR) this.following = true;
		else if (top < this.#lastTop) this.following = false;
		this.#lastTop = top;
	}

	/** Puts `scroller` at its bottom. */
	toBottom(scroller: HTMLElement | undefined): void {
		if (scroller) scroller.scrollTop = scroller.scrollHeight;
	}

	/**
	 * Keeps a following reader at the bottom whatever grows `content` (a
	 * message, a reply, the working line, a refetch) or shrinks `scroller`.
	 * Returns what stops it.
	 */
	observe(scroller: HTMLElement, content: HTMLElement): () => void {
		const observer = new ResizeObserver(() => {
			if (this.following) this.toBottom(scroller);
		});
		observer.observe(scroller);
		observer.observe(content);
		return () => observer.disconnect();
	}
}
