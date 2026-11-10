<!--
	The one CodeMirror wrapper. Nothing else imports CodeMirror.
	`value` flows in; edits flow out through `onchange`. An incoming `value`
	that differs from the document replaces it (e.g. a refetch), so callers
	should feed back what `onchange` gave them. Markdown can show a
	`cohesion://` link alone on its line as a card (`embed`) and look items up
	after `@` or `[[` to link them (`mentions`); the caller draws the card and does
	the search, so this stays a primitive.
-->
<script lang="ts" module>
	/** Draws link `href`, with text `label`, as a card in `host`; returns what takes it down. */
	export type EmbedMount = (host: HTMLElement, href: string, label: string) => () => void;

	/** An item `@` or `[[` found, linked as `[label](href)` when picked. */
	export interface Mention {
		label: string;
		/** What it is, shown quietly beside it: `Task #42`. */
		detail: string;
		/** Its `cohesion://` link. */
		href: string;
	}

	/** The items whose title matches `query`, best first, at most 8. */
	export type MentionSearch = (query: string) => Promise<Mention[]>;
</script>

<script lang="ts">
	import {
		acceptCompletion,
		autocompletion,
		type CompletionContext,
		type CompletionResult
	} from '@codemirror/autocomplete';
	import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
	import { javascript } from '@codemirror/lang-javascript';
	import { markdown } from '@codemirror/lang-markdown';
	import { HighlightStyle, syntaxHighlighting, syntaxTree } from '@codemirror/language';
	import { Compartment, EditorState, Facet, Prec, RangeSetBuilder } from '@codemirror/state';
	import {
		Decoration,
		EditorView,
		ViewPlugin,
		WidgetType,
		keymap,
		placeholder as placeholderExt,
		type DecorationSet,
		type ViewUpdate
	} from '@codemirror/view';
	import { tags } from '@lezer/highlight';
	import { onDestroy, onMount } from 'svelte';
	import { embedLine, markdownLink } from '$lib/format/itemLinks';
	import { mentionTrigger } from '$lib/format/mentionTrigger';
	import { SEARCH_DEBOUNCE_MS } from '$lib/format/search';

	interface Props {
		value: string;
		language?: 'markdown' | 'javascript';
		onchange?: (value: string) => void;
		onblur?: () => void;
		/** Runs on ⌘/Ctrl+Enter in place of the editor's own binding, e.g. a form's Save. */
		onsubmit?: () => void;
		placeholder?: string;
		/** Accessible name for the editing surface. */
		label?: string;
		/** Selection, copy and search still work; typing doesn't. */
		readonly?: boolean;
		/** The surface's minimum height, so a short document still has room to click into. */
		minHeight?: string;
		/** Grows to fill its flex column instead, `minHeight` aside. */
		fill?: boolean;
		/** `sm`: body size with looser leading and paragraph gaps, for long prose such as the journal. */
		size?: 'sm' | 'md';
		/** The document's own title, drawn above: a first-line heading saying only it is hidden, as the title shows it. */
		title?: string;
		/** Markdown only: shows a `cohesion://` link alone on a line the caret isn't on as a card. */
		embed?: EmbedMount;
		/** Markdown only: after `@` or `[[` typed at a word's start, looks items up to link. */
		mentions?: MentionSearch;
	}

	let {
		value,
		language = 'markdown',
		onchange,
		onblur,
		onsubmit,
		placeholder = '',
		label = 'Editor',
		readonly = false,
		minHeight = '50vh',
		fill = false,
		size = 'md',
		title = '',
		embed,
		mentions
	}: Props = $props();

	let host: HTMLDivElement;
	let view: EditorView | null = null;
	const languageSlot = new Compartment();
	const readOnlySlot = new Compartment();
	const titleSlot = new Compartment();
	/** The title the page draws above the text (`title`). */
	const titleFacet = Facet.define<string, string>({ combine: (values) => values[0] ?? '' });
	// A line out of the flow, where the title already says it.
	const hiddenLine = Decoration.line({ class: 'cm-hidden-line' });

	// Colors come from the tokens, so light and dark follow the page.
	const theme = EditorView.theme({
		'&': {
			color: 'var(--fg)',
			backgroundColor: 'transparent',
			fontSize: 'var(--editor-size)',
			// Fills the host, so a click anywhere in it lands in the text.
			flexGrow: '1'
		},
		'&.cm-focused': { outline: 'none' },
		'.cm-scroller': { fontFamily: 'inherit', lineHeight: 'var(--editor-leading)' },
		'.cm-content': { padding: '0', caretColor: 'var(--accent)' },
		// A gap under each line sets paragraphs apart; wrapped text keeps the leading.
		'.cm-line': { padding: '0 0 var(--editor-paragraph)' },
		'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--accent)' },
		'&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection':
			{ backgroundColor: 'var(--selection)' },
		'.cm-placeholder': { color: 'var(--fg-subtle)' },
		// A full-size disc in a fixed gutter, half the text's weight in color.
		'.cm-bullet': {
			display: 'inline-block',
			width: '1em',
			color: 'var(--fg-muted)',
			fontSize: '1.15em',
			lineHeight: '1'
		},
		'.cm-activeLine': { backgroundColor: 'transparent' },
		'.cm-hidden-line': { display: 'none' },
		'.cm-inline-code': {
			backgroundColor: 'var(--bg-hover)',
			borderRadius: '4px',
			padding: '0.1em 0.3em'
		},
		// A card stands in for its line's text, the line's own gap below it.
		// Inline-block, so the line holds only the card and no empty line box beside it;
		// contained, so a long title is cut rather than widening every line.
		'.cm-embed': {
			display: 'inline-block',
			width: '100%',
			contain: 'inline-size',
			verticalAlign: 'top',
			padding: 'var(--s1) 0',
			lineHeight: 'normal',
			cursor: 'default'
		},
		// The picker's list, as the app's menus look.
		'.cm-tooltip.cm-tooltip-autocomplete': {
			minWidth: '18rem',
			maxWidth: '28rem',
			padding: 'var(--s1)',
			border: '1px solid var(--border)',
			borderRadius: 'var(--radius-sm)',
			backgroundColor: 'var(--bg-raised)',
			boxShadow: 'var(--shadow-floating)',
			fontFamily: 'var(--font-sans)'
		},
		'.cm-tooltip.cm-tooltip-autocomplete > ul': { maxHeight: 'none', fontFamily: 'inherit' },
		'.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
			display: 'flex',
			alignItems: 'baseline',
			gap: 'var(--s3)',
			padding: 'var(--s1) var(--s2)',
			borderRadius: 'var(--radius-sm)',
			fontSize: 'var(--text-sm)',
			lineHeight: '1.5',
			color: 'var(--fg)'
		},
		'.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
			backgroundColor: 'var(--bg-hover)',
			color: 'var(--fg)'
		},
		'.cm-completionLabel': {
			minWidth: '0',
			overflow: 'hidden',
			textOverflow: 'ellipsis',
			whiteSpace: 'nowrap'
		},
		'.cm-completionDetail': {
			marginLeft: 'auto',
			flex: 'none',
			fontStyle: 'normal',
			fontSize: 'var(--text-xs)',
			color: 'var(--fg-subtle)'
		},
		'.cm-completionMatchedText': { textDecoration: 'none' }
	});

	const monoTheme = EditorView.theme({
		'&': { fontSize: 'var(--text-sm)' },
		'.cm-scroller': { fontFamily: 'var(--font-mono)' }
	});

	const highlight = HighlightStyle.define([
		{ tag: tags.heading1, fontSize: '1.4em', fontWeight: '600' },
		{ tag: tags.heading2, fontSize: '1.2em', fontWeight: '600' },
		{ tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontWeight: '600' },
		{ tag: tags.strong, fontWeight: '600' },
		{ tag: tags.emphasis, fontStyle: 'italic' },
		{ tag: tags.strikethrough, textDecoration: 'line-through' },
		{ tag: [tags.link, tags.url], color: 'var(--accent)' },
		{ tag: tags.monospace, fontFamily: 'var(--font-mono)', fontSize: '0.92em' },
		{ tag: [tags.processingInstruction, tags.meta, tags.quote], color: 'var(--fg-subtle)' },
		{ tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: 'var(--accent)' },
		{ tag: [tags.string, tags.special(tags.string)], color: 'var(--ok)' },
		{ tag: [tags.number, tags.bool, tags.null], color: 'var(--warn)' },
		{ tag: [tags.comment, tags.lineComment, tags.blockComment], color: 'var(--fg-subtle)' },
		{ tag: [tags.function(tags.variableName), tags.propertyName], color: 'var(--fg)' },
		{ tag: tags.invalid, color: 'var(--danger)' }
	]);

	// CodeMirror switches off the browser's text assistance; prose wants it back.
	const proseAttributes = EditorView.contentAttributes.of({
		spellcheck: 'true',
		autocorrect: 'on',
		autocapitalize: 'sentences',
		writingsuggestions: 'true'
	});

	// A bullet list's "-", "*" or "+" drawn as a bullet, so a list reads as a list while the
	// source stays Markdown; the mark is one character, so the cursor steps over it as before.
	// Drawn by `livePreview` below.
	class Bullet extends WidgetType {
		toDOM() {
			const mark = document.createElement('span');
			mark.className = 'cm-bullet';
			mark.textContent = '•';
			return mark;
		}
		eq() {
			return true;
		}
	}
	const bullet = Decoration.replace({ widget: new Bullet() });

	// Inline code, backticks and all, on a quiet tint, as rendered Markdown draws it.
	const inlineCode = Decoration.mark({ class: 'cm-inline-code' });
	const hidden = Decoration.replace({});

	// A link alone on its line, as a card the caller draws (`embed`). The card keeps the
	// clicks it gets; the editor re-measures when it resizes, as it resolves.
	class EmbedWidget extends WidgetType {
		#takeDown = new WeakMap<HTMLElement, () => void>();
		readonly href: string;
		readonly label: string;
		readonly mount: EmbedMount;
		constructor(href: string, label: string, mount: EmbedMount) {
			super();
			this.href = href;
			this.label = label;
			this.mount = mount;
		}
		eq(other: EmbedWidget) {
			return other.href === this.href && other.label === this.label;
		}
		get estimatedHeight() {
			return 64;
		}
		toDOM(view: EditorView) {
			const host = document.createElement('div');
			host.className = 'cm-embed';
			const unmount = this.mount(host, this.href, this.label);
			const resized = new ResizeObserver(() => view.requestMeasure());
			resized.observe(host);
			this.#takeDown.set(host, () => {
				resized.disconnect();
				unmount();
			});
			return host;
		}
		destroy(dom: HTMLElement) {
			this.#takeDown.get(dom)?.();
		}
	}

	/** Whether `pos` is in code, where a link is text and `@` is just a character. */
	function inCode(state: EditorState, pos: number): boolean {
		const code = ['FencedCode', 'CodeBlock', 'InlineCode', 'CodeText'];
		for (let node = syntaxTree(state).resolveInner(pos, -1); ;) {
			if (code.includes(node.name)) return true;
			if (!node.parent) return false;
			node = node.parent;
		}
	}

	/** The lines the selection touches, where the source shows as typed so it can be edited. */
	function editingLines(view: EditorView): (line: number) => boolean {
		if (!view.hasFocus) return () => false;
		const spans = view.state.selection.ranges.map((range) => [
			view.state.doc.lineAt(range.from).number,
			view.state.doc.lineAt(range.to).number
		]);
		return (line) => spans.some(([first, last]) => line >= first && line <= last);
	}

	/**
	 * Markdown read as it renders, Live Preview style: a list's mark as a bullet always, and away
	 * from the lines being edited, a heading's "#"s, code's backticks and emphasis's asterisks
	 * hidden. The source stays Markdown; on a line the caret is in, every mark shows as typed.
	 */
	function markdownMarks(view: EditorView): DecorationSet {
		const marks: { from: number; to: number; deco: Decoration }[] = [];
		const editing = editingLines(view);
		const doc = view.state.doc;
		// A first line that is the title as a heading, and the blank line after it, until the caret goes there.
		const title = view.state.facet(titleFacet).trim();
		if (title !== '' && doc.lines > 1 && !editing(1) && !editing(2)) {
			const first = doc.line(1);
			if (/^#{1,2} /.test(first.text) && first.text.replace(/^#+ /, '').trim() === title) {
				marks.push({ from: first.from, to: first.from, deco: hiddenLine });
				const next = doc.line(2);
				if (next.text.trim() === '')
					marks.push({ from: next.from, to: next.from, deco: hiddenLine });
			}
		}
		for (const { from, to } of view.visibleRanges) {
			syntaxTree(view.state).iterate({
				from,
				to,
				enter: (node) => {
					const away = !editing(doc.lineAt(node.from).number);
					if (node.name === 'ListMark') {
						const mark = doc.sliceString(node.from, node.to);
						if (mark === '-' || mark === '*' || mark === '+')
							marks.push({ from: node.from, to: node.to, deco: bullet });
					} else if (node.name === 'InlineCode') {
						marks.push({ from: node.from, to: node.to, deco: inlineCode });
					} else if (node.name === 'HeaderMark' && away) {
						// The "#"s and the space after them.
						const end = doc.sliceString(node.to, node.to + 1) === ' ' ? node.to + 1 : node.to;
						marks.push({ from: node.from, to: end, deco: hidden });
					} else if ((node.name === 'CodeMark' || node.name === 'EmphasisMark') && away) {
						// Only inline marks: a fenced block's fence keeps its line.
						if (node.node.parent?.name !== 'FencedCode')
							marks.push({ from: node.from, to: node.to, deco: hidden });
					}
				}
			});
		}
		// A link alone on a line the caret isn't on shows as its card, over the line's other marks.
		if (embed) {
			const cards: { from: number; to: number }[] = [];
			for (const { from, to } of view.visibleRanges) {
				for (let pos = from; pos <= to;) {
					const line = doc.lineAt(pos);
					const found = embedLine(line.text);
					if (found && !editing(line.number) && !inCode(view.state, line.from + 1)) {
						cards.push(line);
						marks.push({
							from: line.from,
							to: line.to,
							deco: Decoration.replace({
								widget: new EmbedWidget(found.href, found.label, embed)
							})
						});
					}
					pos = line.to + 1;
				}
			}
			const covered = (m: { from: number; to: number; deco: Decoration }) =>
				cards.some(
					(c) => m.from >= c.from && m.to <= c.to && !(m.deco.spec.widget instanceof EmbedWidget)
				);
			for (let i = marks.length - 1; i >= 0; i--) if (covered(marks[i])) marks.splice(i, 1);
		}
		// A mark decoration and a replace inside it may start at one place; the builder wants them sorted.
		marks.sort((x, y) => x.from - y.from || x.deco.startSide - y.deco.startSide);
		const set = new RangeSetBuilder<Decoration>();
		for (const m of marks) set.add(m.from, m.to, m.deco);
		return set.finish();
	}

	const livePreview = ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(view: EditorView) {
				this.decorations = markdownMarks(view);
			}
			update(update: ViewUpdate) {
				// The tree also grows after an edit, as the parser catches up.
				const parsed = syntaxTree(update.startState) !== syntaxTree(update.state);
				if (
					update.docChanged ||
					update.viewportChanged ||
					update.selectionSet ||
					update.focusChanged ||
					update.startState.facet(titleFacet) !== update.state.facet(titleFacet) ||
					parsed
				) {
					this.decorations = markdownMarks(update.view);
				}
			}
		},
		{ decorations: (plugin) => plugin.decorations }
	);

	/** Waits `ms`, so a search starts only once typing pauses. */
	const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

	/**
	 * Items to link after an `@` or a `[[` that starts a word, once a character follows
	 * it, outside code. Picking one puts its Markdown link in place of the opener and what
	 * was typed; on a line of its own, that link shows as its card.
	 */
	async function mentionSource(context: CompletionContext): Promise<CompletionResult | null> {
		const search = mentions;
		if (!search) return null;
		const line = context.state.doc.lineAt(context.pos);
		const trigger = mentionTrigger(context.state.sliceDoc(line.from, context.pos));
		if (!trigger) return null;
		const at = line.from + trigger.at;
		if (inCode(context.state, at)) return null;
		const query = trigger.query;
		await pause(SEARCH_DEBOUNCE_MS);
		if (context.aborted) return null;
		const found = await search(query).catch(() => []);
		if (context.aborted || found.length === 0) return null;
		return {
			from: at,
			filter: false,
			options: found.map((mention) => ({
				label: mention.label,
				detail: mention.detail,
				apply: markdownLink(mention.label, mention.href)
			}))
		};
	}

	const mentionCompletion = [
		autocompletion({
			override: [mentionSource],
			icons: false,
			maxRenderedOptions: 8,
			closeOnBlur: true
		}),
		Prec.high(keymap.of([{ key: 'Tab', run: acceptCompletion }]))
	];

	function languageExtension(lang: 'markdown' | 'javascript') {
		return lang === 'markdown'
			? [markdown(), proseAttributes, livePreview, mentions ? mentionCompletion : []]
			: [javascript({ typescript: true }), monoTheme];
	}

	function readOnlyExtension(on: boolean) {
		return [EditorState.readOnly.of(on), EditorView.editable.of(!on)];
	}

	function submit(): boolean {
		if (!onsubmit) return false;
		onsubmit();
		return true;
	}

	onMount(() => {
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc: value,
				extensions: [
					Prec.highest(keymap.of([{ key: 'Mod-Enter', run: submit }])),
					history(),
					keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
					EditorView.lineWrapping,
					placeholderExt(placeholder),
					theme,
					syntaxHighlighting(highlight),
					languageSlot.of(languageExtension(language)),
					readOnlySlot.of(readOnlyExtension(readonly)),
					titleSlot.of(titleFacet.of(title)),
					EditorView.contentAttributes.of({ 'aria-label': label }),
					EditorView.updateListener.of((update) => {
						if (update.docChanged) onchange?.(update.state.doc.toString());
					}),
					EditorView.domEventHandlers({
						blur: () => {
							onblur?.();
						}
					})
				]
			})
		});
	});

	$effect(() => {
		const next = value;
		if (view && next !== view.state.doc.toString()) {
			view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
		}
	});

	$effect(() => {
		const lang = language;
		view?.dispatch({ effects: languageSlot.reconfigure(languageExtension(lang)) });
	});

	$effect(() => {
		const next = title;
		view?.dispatch({ effects: titleSlot.reconfigure(titleFacet.of(next)) });
	});

	$effect(() => {
		const on = readonly;
		view?.dispatch({ effects: readOnlySlot.reconfigure(readOnlyExtension(on)) });
	});

	onDestroy(() => view?.destroy());

	/** Moves keyboard focus into the editor. */
	export function focus(): void {
		view?.focus();
	}
</script>

<div
	class={['flex flex-col', readonly ? 'cursor-auto' : 'cursor-text', fill && 'flex-1']}
	style:min-height={fill ? undefined : minHeight}
	style:--editor-size={size === 'sm' ? 'var(--text-sm)' : 'var(--text-md)'}
	style:--editor-leading={size === 'sm' ? '1.55' : 'var(--leading)'}
	style:--editor-paragraph={size === 'sm' ? '0.25em' : '0'}
	bind:this={host}
></div>
