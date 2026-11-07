<!--
	The one CodeMirror wrapper. Nothing else imports CodeMirror.
	`value` flows in; edits flow out through `onchange`. An incoming `value`
	that differs from the document replaces it (e.g. a refetch), so callers
	should feed back what `onchange` gave them.
-->
<script lang="ts">
	import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
	import { javascript } from '@codemirror/lang-javascript';
	import { markdown } from '@codemirror/lang-markdown';
	import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
	import { Compartment, EditorState, Prec } from '@codemirror/state';
	import { EditorView, keymap, placeholder as placeholderExt } from '@codemirror/view';
	import { tags } from '@lezer/highlight';
	import { onDestroy, onMount } from 'svelte';

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
		size = 'md'
	}: Props = $props();

	let host: HTMLDivElement;
	let view: EditorView | null = null;
	const languageSlot = new Compartment();
	const readOnlySlot = new Compartment();

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
		'.cm-activeLine': { backgroundColor: 'transparent' }
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

	function languageExtension(lang: 'markdown' | 'javascript') {
		return lang === 'markdown'
			? [markdown(), proseAttributes]
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
