import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { blinkOnPageClock, shimmerOnPageClock } from './motion';

type Animated = Element & { animate: ReturnType<typeof vi.fn> };

const eye = () => ({ animate: vi.fn(() => ({})) }) as unknown as Animated;

describe('the page clock', () => {
	let stops: (() => void)[] = [];
	const attach = (element: Element) => stops.push(blinkOnPageClock(element));
	const shimmer = (element: Element) => stops.push(shimmerOnPageClock(element));

	beforeEach(() => {
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'], now: 0 });
		vi.stubGlobal('document', { hidden: false });
	});

	afterEach(() => {
		for (const stop of stops) stop();
		stops = [];
		vi.useRealTimers();
		vi.unstubAllGlobals();
	});

	it('blinks an eye that joined mid-cycle on the same boundary as the rest', () => {
		const first = eye();
		const late = eye();
		attach(first);
		vi.advanceTimersByTime(3200);
		attach(late);
		expect(first.animate).not.toHaveBeenCalled();

		vi.advanceTimersByTime(5000 - performance.now());
		expect(first.animate).toHaveBeenCalledTimes(1);
		expect(late.animate).toHaveBeenCalledTimes(1);

		vi.advanceTimersByTime(5000);
		expect(first.animate).toHaveBeenCalledTimes(2);
		expect(late.animate).toHaveBeenCalledTimes(2);
	});

	it('stops blinking an eye once detached, and rests while the page is hidden', () => {
		const stays = eye();
		const leaves = eye();
		attach(stays);
		const stop = blinkOnPageClock(leaves);
		stop();
		vi.advanceTimersByTime(5000);
		expect(leaves.animate).not.toHaveBeenCalled();
		expect(stays.animate).toHaveBeenCalledTimes(1);

		vi.stubGlobal('document', { hidden: true });
		vi.advanceTimersByTime(5000);
		expect(stays.animate).toHaveBeenCalledTimes(1);
	});

	it('shimmers on every beat and blinks on every second one, in the same tick', () => {
		const text = eye();
		const blinker = eye();
		vi.advanceTimersByTime(1000);
		shimmer(text);
		attach(blinker);

		vi.advanceTimersByTime(1500);
		expect(text.animate).toHaveBeenCalledTimes(1);
		expect(blinker.animate).not.toHaveBeenCalled();

		vi.advanceTimersByTime(2500);
		expect(text.animate).toHaveBeenCalledTimes(2);
		expect(blinker.animate).toHaveBeenCalledTimes(1);

		vi.advanceTimersByTime(5000);
		expect(text.animate).toHaveBeenCalledTimes(4);
		expect(blinker.animate).toHaveBeenCalledTimes(2);
	});

	it('gives text mounted during a crossing the rest of it, from the beat', () => {
		const first = eye();
		vi.advanceTimersByTime(1000);
		shimmer(first);
		vi.advanceTimersByTime(4300);

		const during = eye();
		shimmer(during);
		expect(during.animate).toHaveBeenCalledTimes(1);
		expect(during.animate.mock.results[0].value.startTime).toBe(5000);

		vi.advanceTimersByTime(700);
		const between = eye();
		shimmer(between);
		expect(between.animate).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1500);
		expect(first.animate).toHaveBeenCalledTimes(3);
		expect(during.animate).toHaveBeenCalledTimes(2);
		expect(between.animate).toHaveBeenCalledTimes(1);
	});

	it('keeps its phase after everything has left and something joins again', () => {
		shimmer(eye());
		vi.advanceTimersByTime(1000);
		stops.pop()?.();

		vi.advanceTimersByTime(2000);
		const later = eye();
		shimmer(later);
		expect(later.animate).toHaveBeenCalledTimes(1);
		expect(later.animate.mock.results[0].value.startTime).toBe(2500);
		vi.advanceTimersByTime(2000);
		expect(later.animate).toHaveBeenCalledTimes(2);
	});
});
