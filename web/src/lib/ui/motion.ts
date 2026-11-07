// The durations every Svelte transition and `animate:` runs for.
//
// CSS state changes (colour, hover, overlays, page fades) run on the
// `--duration` token in tokens.css, 120ms. Svelte enter and leave
// transitions and `animate:` run longer, for 200ms, because they move or
// resize content. A column sliding in beside the page runs as the side
// panel does in tokens.css: in for 180ms on a steep ease-out, out for 140ms
// on an accelerating curve.

import { cubicIn, expoOut } from 'svelte/easing';
import { prefersReducedMotion } from 'svelte/motion';

const DURATION = 200;
const SLIDE_IN = 180;
const SLIDE_OUT = 140;

const reduced = (ms: number) => (prefersReducedMotion.current ? 0 : ms);

// The page's one animation clock: a beat every 2.5s of the page's time. Each beat
// starts a shimmer's pulse, and every second one a blink as well, so nothing that
// repeats keeps a clock of its own, and a phase depends only on the time.
const BEAT = 2500;
const BLINK_CYCLE = 2 * BEAT;

const BLINK_LENGTH = 440;
/**
 * Two blinks 80ms apart: each squeezes to a slit in 80ms, speeding up, and opens
 * in 100ms, slowing down, as an eyelid does.
 */
const BLINK: Keyframe[] = [
	{ offset: 0, transform: 'scaleY(1)', easing: 'ease-in' },
	{ offset: 80 / BLINK_LENGTH, transform: 'scaleY(0.1)', easing: 'ease-out' },
	{ offset: 180 / BLINK_LENGTH, transform: 'scaleY(1)' },
	{ offset: 260 / BLINK_LENGTH, transform: 'scaleY(1)', easing: 'ease-in' },
	{ offset: 340 / BLINK_LENGTH, transform: 'scaleY(0.1)', easing: 'ease-out' },
	{ offset: 1, transform: 'scaleY(1)' }
];

const SHIMMER_LENGTH = 900;
/**
 * The band of Shimmer.svelte's mask crossing its text, left to right. It sets off
 * just clear of the text, so the crossing shows as soon as the beat falls, and it
 * ends clear of it, so the mask at rest looks the same.
 */
const SHIMMER: Keyframe[] = [{ maskPosition: '95% 0' }, { maskPosition: '5% 0' }];
const SHIMMER_TIMING = { duration: SHIMMER_LENGTH, easing: 'ease-out' };

const blinkingEyes = new Set<Element>();
const shimmeringText = new Set<Element>();
let nextBeat = 0;
let beatTimer: ReturnType<typeof setTimeout>;

const resting = () => document.hidden || prefersReducedMotion.current;

function scheduleBeat() {
	const now = performance.now();
	// Past the beat just served, even if the timer fired a hair before it.
	nextBeat = Math.max(nextBeat + BEAT, Math.ceil(now / BEAT) * BEAT);
	beatTimer = setTimeout(() => {
		if (!resting()) {
			for (const text of shimmeringText) text.animate(SHIMMER, SHIMMER_TIMING);
			if (nextBeat % BLINK_CYCLE === 0)
				for (const eye of blinkingEyes) eye.animate(BLINK, BLINK_LENGTH);
		}
		scheduleBeat();
	}, nextBeat - now);
}

function joinPageClock(joined: Set<Element>, element: Element) {
	joined.add(element);
	if (blinkingEyes.size + shimmeringText.size === 1) scheduleBeat();
	return () => {
		joined.delete(element);
		if (blinkingEyes.size + shimmeringText.size > 0) return;
		clearTimeout(beatTimer);
		nextBeat = 0;
	};
}

/**
 * An attachment that blinks an element on every second beat of the page clock.
 * One timer starts everything on a beat in the same tick, so the engine starts it
 * all together and nothing can drift; an eye that joins or shows again just takes
 * the next blink, or the rest of this one.
 */
export function blinkOnPageClock(eye: Element) {
	return joinPageClock(blinkingEyes, eye);
}

/**
 * An attachment that shimmers an element on every beat of the page clock. Text
 * mounted during a crossing takes the rest of it, so a row that re-renders stays
 * in step with the others instead of restarting. It sets this crossing's start,
 * which only suits what the main thread animates: a shimmer's mask, not a blink.
 */
export function shimmerOnPageClock(text: Element) {
	const now = performance.now();
	const sinceBeat = now % BEAT;
	if (sinceBeat < SHIMMER_LENGTH && !resting())
		text.animate(SHIMMER, SHIMMER_TIMING).startTime = now - sinceBeat;
	return joinPageClock(shimmeringText, text);
}

export const motion = {
	/**
	 * How long a Svelte transition runs, in ms: 200, or 0 under reduced
	 * motion. Svelte transitions run in JS, so the reduced-motion CSS rule in
	 * tokens.css can't reach them.
	 */
	get duration(): number {
		return reduced(DURATION);
	},
	/** A column's `in:` transition: 180ms on a steep ease-out, or 0 under reduced motion. */
	get slideIn() {
		return { duration: reduced(SLIDE_IN), easing: expoOut };
	},
	/** A column's `out:` transition: 140ms, accelerating, or 0 under reduced motion. */
	get slideOut() {
		return { duration: reduced(SLIDE_OUT), easing: cubicIn };
	}
};
