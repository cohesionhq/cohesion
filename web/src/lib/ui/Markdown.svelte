<script lang="ts">
	import DOMPurify from 'dompurify';
	import { marked, type Token } from 'marked';
	import { connection } from '$lib/protocol/api';
	import { textPieces } from '$lib/format/itemLinks';
	import EmbedCard from '$lib/shell/EmbedCard.svelte';
	import { linkedItem } from '$lib/shell/linkedItem.svelte';
	import { itemLinkRoute, parseItemLink, type ItemLink } from '$lib/shell/routes';
	import { diagrams as drawings, fenceClosed } from './diagram';
	import ImageViewer, { type ViewerItem } from './ImageViewer.svelte';

	interface Props {
		source: string;
		/** Draws ```mermaid blocks as diagrams; a block that won't draw stays code. */
		diagrams?: boolean;
		/** Shows a `cohesion://` link alone on its line as a card of its item (`textPieces`). */
		embeds?: boolean;
	}

	let { source, diagrams = false, embeds = false }: Props = $props();

	let root = $state<HTMLElement>();

	/** The text and its cards in order; all text without `embeds`. */
	const pieces = $derived(
		(embeds ? textPieces(source) : [{ kind: 'markdown' as const, text: source }]).map((piece) =>
			piece.kind === 'markdown' ? { ...piece, ...render(piece.text) } : piece
		)
	);
	/** The item each in-app href in the text opens. */
	const links = $derived(
		Object.assign(
			{},
			...pieces.map((piece) => (piece.kind === 'markdown' ? piece.links : {}))
		) as Record<string, ItemLink>
	);

	/**
	 * The sanitized HTML, and the item each in-app href in it opens. A link to
	 * an item (`cohesion://task/3/42`) points at its Open page, which a plain
	 * click leaves for the item's panel; DOMPurify drops any other `cohesion:`
	 * href, leaving its text. A fence still open loses its language, so it stays
	 * code until it closes.
	 */
	function render(markdown: string): { html: string; links: Record<string, ItemLink> } {
		const links: Record<string, ItemLink> = {};
		const walkTokens = (token: Token) => {
			if (token.type === 'code' && !fenceClosed(token.raw)) token.lang = '';
			if (token.type !== 'link') return;
			const link = parseItemLink(token.href);
			if (link === null) return;
			token.href = itemLinkRoute(link);
			links[token.href] = link;
		};
		const html = marked.parse(markdown, { async: false, gfm: true, walkTokens });
		return { html: DOMPurify.sanitize(html), links };
	}

	$effect(() => {
		// A reconnect runs this again: a reply the old socket lost is asked for anew.
		void connection.status;
		if (!diagrams || !root || pieces.length === 0) return;
		for (const code of root.querySelectorAll('code[class="language-mermaid" i]')) {
			const block = code.closest('pre');
			if (block) void draw(block, code.textContent ?? '');
		}
	});

	/** Swaps a code block for its diagram, unless the text has moved on or it won't draw. */
	async function draw(block: HTMLPreElement, text: string) {
		const width = block.clientWidth;
		const drawing = await drawings.draw(text, width);
		if (drawing === null || !block.isConnected) return;
		const figure = document.createElement('div');
		figure.className = 'diagram';
		figure.dataset.source = text;
		figure.dataset.width = String(width);
		figure.innerHTML = drawing.svg;
		const expand = document.createElement('button');
		expand.type = 'button';
		expand.className = 'diagram-expand';
		// Lucide's maximize-2, drawn by hand: the button is built outside Svelte.
		expand.innerHTML =
			'<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 3h6v6"/><path d="m21 3-7 7"/><path d="m3 21 7-7"/><path d="M9 21H3v-6"/></svg>';
		expand.setAttribute('aria-label', 'Expand diagram');
		figure.append(expand);
		block.replaceWith(figure);
	}

	let viewing = $state<ViewerItem | null>(null);

	/**
	 * An image clicked, unless it is a link's, opens near full screen. A link
	 * to an item opens it over the page (`linkedItem.follow`).
	 */
	function onclick(event: MouseEvent) {
		const target = event.target;
		if (!(target instanceof Element)) return;
		const anchor = target.closest('a');
		const figure = target.closest<HTMLElement>('.diagram');
		const drawing = figure?.querySelector(':scope > svg');
		if (figure && drawing && !anchor)
			viewing = {
				diagram: figure.dataset.source ?? '',
				shownAt: Number(figure.dataset.width),
				alt: 'Diagram'
			};
		if (target instanceof HTMLImageElement && !anchor)
			viewing = { src: target.src, alt: target.alt };
		const href = anchor?.getAttribute('href');
		if (href && Object.hasOwn(links, href)) linkedItem.follow(event, links[href]);
	}
</script>

<!-- Links are reached by keyboard themselves, and Enter clicks them; an image has no keyboard way in. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div bind:this={root} class="markdown leading-(--leading) wrap-anywhere" {onclick}>
	{#each pieces as piece, index (index)}
		{#if piece.kind === 'markdown'}
			<!-- eslint-disable-next-line svelte/no-at-html-tags -- sanitized by DOMPurify above -->
			{@html piece.html}
		{:else}
			<div class="embed"><EmbedCard href={piece.href} label={piece.label} /></div>
		{/if}
	{/each}
</div>

<ImageViewer bind:image={viewing} />

<!-- The rendered HTML isn't markup here, so its elements are styled below. -->
<style>
	.markdown :global(:first-child) {
		margin-top: 0;
	}

	.markdown :global(:last-child) {
		margin-bottom: 0;
	}

	.markdown :global(:is(h1, h2, h3, h4, h5, h6)) {
		margin: 1.25em 0 0.5em;
	}

	.markdown :global(h1) {
		font-size: var(--text-lg);
	}

	.markdown :global(h2) {
		font-size: var(--text-md);
	}

	.markdown :global(p),
	.markdown :global(ul),
	.markdown :global(ol),
	.markdown :global(pre),
	.markdown :global(blockquote),
	.markdown :global(table),
	.markdown :global(hr),
	.markdown .embed {
		margin: 0 0 0.75em;
	}

	.markdown :global(ul),
	.markdown :global(ol) {
		padding-left: var(--s6);
	}

	.markdown :global(ul) {
		list-style: disc;
	}

	.markdown :global(ol) {
		list-style: decimal;
	}

	.markdown :global(:is(th, td)) {
		padding: var(--s1) var(--s2);
	}

	.markdown :global(pre) {
		padding: var(--s3);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--bg-raised);
		overflow-x: auto;
	}

	/* The diagram takes the page's colors through the variables the SVG reads,
	   which its own inline style would otherwise set to fixed ones. */
	.markdown :global(.diagram) {
		position: relative;
		margin: 0 0 0.75em;
		overflow-x: auto;
		cursor: zoom-in;
	}

	.markdown :global(.diagram-expand) {
		position: absolute;
		top: var(--s1);
		right: var(--s1);
		display: inline-flex;
		padding: var(--s2);
		border-radius: var(--radius-sm);
		background: var(--bg-raised);
		color: var(--fg-muted);
		cursor: pointer;
		opacity: 0;
		transition: opacity var(--duration) ease-out;
	}

	.markdown :global(.diagram:hover .diagram-expand),
	.markdown :global(.diagram-expand:focus-visible) {
		opacity: 1;
	}

	@media (hover: none) {
		.markdown :global(.diagram-expand) {
			opacity: 1;
		}
	}

	.markdown :global(.diagram svg) {
		max-width: 100%;
		height: auto;
	}

	.markdown :global(blockquote) {
		padding-left: var(--s3);
		border-left: 2px solid var(--border-strong);
		color: var(--fg-muted);
	}

	.markdown :global(img) {
		max-width: 50%;
		max-height: 50dvh;
		border-radius: var(--radius-sm);
		opacity: 0.7;
		transition: opacity var(--duration) ease-out;
		cursor: zoom-in;
	}

	.markdown :global(a img) {
		cursor: pointer;
	}

	.markdown :global(img:hover) {
		opacity: 1;
	}

	.markdown :global(hr) {
		border-color: var(--border);
	}
</style>
