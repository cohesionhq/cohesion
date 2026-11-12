import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScrollMemory } from './scrollMemory';

class FakeObserver {
	static last: FakeObserver;
	constructor(private cb: () => void) {
		FakeObserver.last = this;
	}
	live = true;
	observe() {}
	disconnect() {
		this.live = false;
	}
	fire() {
		if (this.live) this.cb();
	}
}

/** A scroller whose content is `height` tall in a 100px window. */
function scroller(height: number) {
	const state = { height };
	let top = 0;
	const listeners = new Map<string, () => void>();
	const el = {
		children: [{}],
		get scrollTop() {
			return top;
		},
		set scrollTop(v: number) {
			top = Math.max(0, Math.min(v, state.height - 100));
		},
		addEventListener: (name: string, fn: () => void) => listeners.set(name, fn),
		removeEventListener: (name: string) => listeners.delete(name),
		dispatchEvent: (event: { type: string }) => listeners.get(event.type)?.()
	};
	return { el: el as unknown as HTMLElement, state };
}

describe('ScrollMemory', () => {
	beforeEach(() => {
		vi.stubGlobal('ResizeObserver', FakeObserver);
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});

	it('returns a page to where it was left and a new page to the top', () => {
		const { el } = scroller(1000);
		const m = new ScrollMemory(() => el);
		m.enter('/a');
		el.scrollTop = 400;
		m.onscroll();
		m.enter('/b');
		expect(el.scrollTop).toBe(0);
		el.scrollTop = 50;
		m.onscroll();
		m.enter('/a');
		expect(el.scrollTop).toBe(400);
		m.enter('/b');
		expect(el.scrollTop).toBe(50);
	});

	it('waits for a loading page to grow tall enough', () => {
		const { el, state } = scroller(1000);
		const m = new ScrollMemory(() => el);
		m.enter('/a');
		el.scrollTop = 600;
		m.onscroll();
		state.height = 150;
		m.enter('/b');
		m.enter('/a');
		expect(el.scrollTop).toBe(50);
		m.onscroll(); // the clamped position is not remembered
		state.height = 1000;
		FakeObserver.last.fire();
		expect(el.scrollTop).toBe(600);
		m.enter('/b');
		m.enter('/a');
		expect(el.scrollTop).toBe(600);
	});

	it('stops restoring once the user scrolls', () => {
		const { el, state } = scroller(150);
		const m = new ScrollMemory(() => el);
		m.enter('/a');
		state.height = 1000;
		el.scrollTop = 600;
		m.onscroll();
		state.height = 150;
		m.enter('/b');
		m.enter('/a');
		(el as unknown as { dispatchEvent(e: object): void }).dispatchEvent({ type: 'wheel' });
		state.height = 1000;
		FakeObserver.last.fire();
		expect(el.scrollTop).toBe(50);
	});
});
