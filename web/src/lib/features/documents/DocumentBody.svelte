<!--
	The open document's text: the Markdown editor, or a read-only preview. In
	both, a `cohesion://` link alone on its line shows as a card, and in the
	editor `@` looks items up in workspace `workspace` to link.
-->
<script lang="ts">
	import { editorLinks } from '$lib/shell/embedCards';
	import type { Draft } from '$lib/stores/draft.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';

	interface Props {
		draft: Draft;
		/** The workspace the text is in, whose items `@` looks up. */
		workspace: number;
		preview: boolean;
		label: string;
		placeholder: string;
		onblur: () => void;
		/** As the Editor's: runs on ⌘/Ctrl+Enter. */
		onsubmit?: () => void;
		/** As the Editor's: fills its flex column, at body size for long prose. */
		fill?: boolean;
		size?: 'sm' | 'md';
		/** As the Editor's: the title drawn above, which a first heading repeating it gives way to. */
		title?: string;
	}

	let {
		draft,
		workspace,
		preview,
		label,
		placeholder,
		onblur,
		onsubmit,
		fill = false,
		size = 'md',
		title
	}: Props = $props();

	let editor = $state<Editor>();
	const links = $derived(editorLinks(workspace));

	/** Moves keyboard focus into the editor, when it is showing. */
	export function focus(): void {
		editor?.focus();
	}
</script>

{#if preview}
	<div class={size === 'sm' ? 'text-sm leading-[1.7]' : 'text-md'}>
		{#if draft.draft.trim() === ''}
			<p class="text-fg-subtle">Nothing written yet.</p>
		{:else}
			<Markdown source={draft.draft} diagrams embeds />
		{/if}
	</div>
{:else}
	<Editor
		bind:this={editor}
		value={draft.draft}
		language="markdown"
		{label}
		{placeholder}
		{fill}
		{size}
		{title}
		embed={links.embed}
		mentions={links.mentions}
		onchange={(text) => draft.edit(text)}
		{onblur}
		{onsubmit}
	/>
{/if}
