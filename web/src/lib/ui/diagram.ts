import DOMPurify from 'dompurify';
import { send as defaultSend } from '$lib/protocol/api';
import type { Send } from '$lib/protocol/types';

/**
 * Mermaid diagrams as SVG, drawn by the daemon (`renderMermaid`). Colors are not
 * passed: the `.diagram` class (`tokens.css`) points the SVG's `--pele-*`
 * variables at the page's own tokens, so a diagram inlined in the page follows
 * light and dark with no request. It scales from its `viewBox`, so a resize
 * needs none either: only a new width to lay out for does.
 */

/** The most the expanded view enlarges a diagram, so a tiny one doesn't become huge. */
const MOST_ENLARGED = 3;

/** How many replies stay, so a page open for weeks doesn't keep every diagram it showed. */
const MOST_KEPT = 200;

/** A diagram as drawn: sanitized SVG and the size it was laid out at. */
export interface Drawing {
	svg: string;
	width: number;
	height: number;
}

function sanitizeSvg(svg: string): string {
	return DOMPurify.sanitize(svg, { USE_PROFILES: { svg: true, svgFilters: true } });
}

export function createDiagrams(send: Send = defaultSend, sanitize = sanitizeSvg) {
	const kept = new Map<string, Promise<Drawing | null>>();

	/**
	 * A refusal stays, since the same text at the same width is refused again,
	 * except `busy`, which says asking later may work. A lost connection isn't
	 * an answer, so nothing stays for it either.
	 */
	async function ask(key: string, source: string, width?: number): Promise<Drawing | null> {
		try {
			const reply = await send('renderMermaid', { source, width, format: 'svg' });
			if (reply.kind === 'svg') return { ...reply, svg: sanitize(reply.svg) };
			if (reply.reason === 'busy') kept.delete(key);
		} catch {
			kept.delete(key);
		}
		return null;
	}

	return {
		/**
		 * One diagram laid out for `width` pixels, asked for once per source and
		 * width, or `null` when it won't draw: the source stays as code. A width
		 * of 0, a block not laid out yet, gets the size its content wants.
		 */
		draw(source: string, width: number): Promise<Drawing | null> {
			const key = `${width}\n${source}`;
			const known = kept.get(key);
			if (known) return known;
			const asked = ask(key, source, width || undefined);
			kept.set(key, asked);
			if (kept.size > MOST_KEPT) kept.delete(kept.keys().next().value as string);
			return asked;
		}
	};
}

export const diagrams = createDiagrams();

/**
 * Whether a fenced code block has its closing fence, given the block's raw text.
 * Until it does it is still streaming in, and nothing asks to draw it.
 */
export function fenceClosed(raw: string): boolean {
	const lines = raw.trimEnd().split('\n');
	const fence = /^ *(`{3,}|~{3,})/.exec(lines[0])?.[1];
	const last = lines[lines.length - 1].trim();
	return fence !== undefined && lines.length > 1 && last.startsWith(fence) && /^[`~]+$/.test(last);
}

/** Fills `element` with a drawing, up to the most it enlarges; it is vector, so CSS keeps it fitting. */
function fill(element: HTMLElement, { svg, width, height }: Drawing) {
	element.innerHTML = svg;
	const drawn = element.querySelector('svg');
	if (!drawn) return;
	drawn.style.width = '100%';
	drawn.style.height = '100%';
	drawn.style.maxWidth = `${width * MOST_ENLARGED}px`;
	drawn.style.maxHeight = `${height * MOST_ENLARGED}px`;
}

/**
 * Shows a diagram in `element` as it was drawn at `shownAt`, enlarged, then as
 * drawn for the element's own width once that reply comes. If it doesn't come,
 * the first stays.
 */
export async function mountDiagram(element: HTMLElement, source: string, shownAt: number) {
	const shown = await diagrams.draw(source, shownAt);
	if (shown) fill(element, shown);
	const larger = await diagrams.draw(source, element.clientWidth);
	if (larger) fill(element, larger);
}
