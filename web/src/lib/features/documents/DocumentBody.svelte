<!-- The open document's text: the Markdown editor, or a read-only preview. -->
<script lang="ts">
	import type { Draft } from '$lib/stores/draft.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';

	interface Props {
		draft: Draft;
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
			<Markdown source={draft.draft} diagrams />
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
		onchange={(text) => draft.edit(text)}
		{onblur}
		{onsubmit}
	/>
{/if}
