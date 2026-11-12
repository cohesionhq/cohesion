import { describe, expect, it } from 'vitest';
import { BottomFollower } from './followBottom';

/** A feed `height` tall in a 400px viewport, scrolled to `top`. */
const at = (top: number, height = 2000) => ({
	scrollTop: top,
	scrollHeight: height,
	clientHeight: 400
});

describe('BottomFollower', () => {
	it('follows from the start', () => {
		expect(new BottomFollower().following).toBe(true);
	});

	it('lets go when the reader scrolls up, and follows again near the bottom', () => {
		const bottom = new BottomFollower();
		bottom.scrolled(at(1600));
		bottom.scrolled(at(1200));
		expect(bottom.following).toBe(false);
		bottom.scrolled(at(1560));
		expect(bottom.following).toBe(true);
	});

	it('keeps following when the feed grows past a jump to the bottom', () => {
		const bottom = new BottomFollower();
		bottom.scrolled(at(1600));
		// The feed grew before the scroll event landed: far from the bottom, but not scrolled up.
		bottom.scrolled(at(1600, 3000));
		expect(bottom.following).toBe(true);
	});

	it('stays let go while the reader scrolls down short of the bottom', () => {
		const bottom = new BottomFollower();
		bottom.scrolled(at(1600));
		bottom.scrolled(at(800));
		bottom.scrolled(at(1000));
		expect(bottom.following).toBe(false);
	});

	it('follows again when asked, and starts over for another conversation', () => {
		const bottom = new BottomFollower();
		bottom.scrolled(at(1600));
		bottom.scrolled(at(800));
		bottom.follow();
		expect(bottom.following).toBe(true);
		bottom.scrolled(at(700));
		expect(bottom.following).toBe(false);
		bottom.reset();
		// A first scroll at the new feed's top isn't a scroll up.
		bottom.scrolled(at(0));
		expect(bottom.following).toBe(true);
	});
});
