<!--
	A document's panel (`ItemPanel`), or a journal entry's: when it last
	changed, then its whole text, read-only, with Open going to its page.
-->
<script lang="ts">
	import Clock from '@lucide/svelte/icons/clock';
	import FileText from '@lucide/svelte/icons/file-text';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import RewindButton from '$lib/features/rewind/RewindButton.svelte';
	import { longDate, longDay, shortTime } from '$lib/format/dates';
	import type { Document } from '$lib/protocol/types';
	import { documentRoute } from '$lib/shell/links';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';

	let { open = $bindable(false), document }: { open?: boolean; document: Document } = $props();

	/** The document given, overridden by what a restore leaves. */
	let shown = $derived(document);

	const journal = $derived(shown.journal_date !== null);
	const title = $derived(shown.journal_date ? longDay(shown.journal_date) : shown.title);
</script>

<ItemPanel
	bind:open
	{title}
	icon={journal ? NotebookPen : FileText}
	kind={journal ? 'Journal entry' : 'Document'}
	href={documentRoute(shown)}
	link={{ kind: 'document', ws: shown.workspace_id, id: shown.id }}
	linkTitle={shown.title}
>
	{#snippet actions()}
		<RewindButton
			item={{ kind: 'document', workspace_id: shown.workspace_id, id: shown.id }}
			{title}
			onrestored={(result) => {
				if (result.kind === 'document') shown = result.document;
			}}
		/>
	{/snippet}
	{#snippet details()}
		<ItemDetail label="Edited" icon={Clock}>
			{longDate(shown.modified)}, {shortTime(shown.modified)}
		</ItemDetail>
	{/snippet}
	{#if shown.contents.trim()}
		<div class="text-md"><Markdown source={shown.contents} diagrams embeds /></div>
	{:else}
		<p class="text-fg-subtle">Nothing written yet.</p>
	{/if}
</ItemPanel>
