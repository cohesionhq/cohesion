<!--
	A table's row's panel (`ItemPanel`): a document whose folder is a table,
	titled with the document's title. Its value for each of the table's
	columns comes first, each changed in place with the table's editor for
	its type (`ValueCell`; a formula's only reads), then its text the way a
	task's panel holds its description: read as Markdown, or written in the
	documents' editor after Edit, saving as it goes. Open goes to the
	document's own page. It follows the row as it changes anywhere
	(`tables.panel`), and closes once the document is deleted or leaves the
	table.
-->
<script lang="ts">
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import TableProperties from '@lucide/svelte/icons/table-properties';
	import { tick, untrack } from 'svelte';
	import DocumentBody from '$lib/features/documents/DocumentBody.svelte';
	import SavedMark from '$lib/features/documents/SavedMark.svelte';
	import { useAutosave } from '$lib/features/documents/autosave.svelte';
	import type { Document } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import type { RowDocumentKey } from '$lib/stores/tables.svelte';
	import { Draft } from '$lib/stores/draft.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import { TYPE_ICONS } from './DocumentTable.svelte';
	import ValueCell from './ValueCell.svelte';
	import { cellValue } from './values';

	interface Props {
		open?: boolean;
		/** The document as its link resolved, shown until its row loads. */
		document: Document;
	}

	let { open = $bindable(false), document: resolved }: Props = $props();

	const { tables, documents } = pageStores();

	const ws = $derived(resolved.workspace_id);
	const id = $derived(resolved.id);
	const folderId = $derived(resolved.folder_id!);

	/** Whether the store's row is the one this panel shows. */
	const current = $derived.by(() => {
		const key = tables.panel.row.key;
		return key?.workspace_id === ws && key.folder_id === folderId && key.document_id === id;
	});
	const entry = $derived(current ? tables.panel.row.value : null);
	const properties = $derived(current ? tables.panel.properties.value : []);
	const title = $derived(entry?.title ?? resolved.title);
	const folderTitle = $derived(
		documents.folders.value.find((f) => f.id === folderId && f.workspace_id === ws)?.title
	);

	// Loads the row each time the panel opens on one, and lets it go once closed.
	$effect(() => {
		const [isOpen, w, f, d] = [open, ws, folderId, id];
		untrack(() => {
			if (isOpen) void tables.openRow(w, f, d);
		});
		return () => {
			if (isOpen) untrack(() => tables.closeRow());
		};
	});

	// A row deleted or moved out of the table anywhere closes its panel.
	$effect(() => {
		if (open && current && tables.panel.row.loaded && tables.panel.row.value === null) open = false;
	});

	// ---- Its values -------------------------------------------------------------

	/** The property whose value is being edited. */
	let editing = $state<number | null>(null);

	// ---- Its text ------------------------------------------------------------------

	/** The document the draft is of; saves go to it. */
	let subject: RowDocumentKey | null = null;
	const draft = new Draft(async (content) => {
		const document = await tables.writeRowDocument(subject!, content);
		return document.modified;
	});
	const save = useAutosave(draft);

	let preview = $state(true);
	let body = $state<DocumentBody>();
	/** The document the panel last started on; null once closed. */
	let shown = $state<string | null>(null);
	const document = $derived(
		current && tables.panelDocument.value?.id === id ? tables.panelDocument.value : null
	);

	// Start from the saved document each time the panel opens on one, once
	// the last one's edits are saved; a refetch while it is open lands unless
	// there are unsaved edits. Closing saves.
	$effect(() => {
		const [isOpen, doc] = [open, document];
		const key = `${ws}/${id}`;
		untrack(() => {
			if (!isOpen) {
				if (shown !== null) save();
				shown = null;
			} else if (doc && key !== shown) {
				shown = key;
				void start({ workspace_id: ws, document_id: id }, doc.contents);
			} else if (doc && doc.contents !== draft.saved) {
				draft.receive(doc.contents, doc.modified);
			}
		});
	});

	async function start(next: RowDocumentKey, contents: string) {
		await draft.flush().catch(() => undefined);
		subject = next;
		draft.reset(contents);
		preview = contents !== '';
	}

	async function edit() {
		preview = false;
		await tick();
		body?.focus();
	}

	function done() {
		preview = true;
		save();
	}

	/** A link into the app goes to its page, or opens its item in this panel's place, so the panel gets out of the way. */
	function onLink(event: MouseEvent) {
		const link = event.target instanceof Element ? event.target.closest('a') : null;
		if (link && link.origin === location.origin) open = false;
	}
</script>

<ItemPanel
	bind:open
	{title}
	icon={TableProperties}
	kind={folderTitle ?? 'Document'}
	href={routes.document(ws, id)}
	link={{ kind: 'document', ws, id }}
>
	{#snippet actions()}
		<SavedMark {draft} />
	{/snippet}
	{#snippet leading()}
		{#if preview}
			<Button onclick={edit}>Edit</Button>
		{:else}
			<Button onclick={done}>Done</Button>
		{/if}
	{/snippet}
	{#snippet details()}
		{#if entry}
			{#each properties as property (property.id)}
				<ItemDetail
					label={property.name}
					icon={property.formula ? SquareFunction : TYPE_ICONS[property.data_type]}
				>
					<ValueCell
						{property}
						value={cellValue(entry, property.id)}
						rowLabel={title}
						look="detail"
						editing={editing === property.id}
						onedit={() => (editing = property.id)}
						onstop={() => {
							if (editing === property.id) editing = null;
						}}
						onsave={(value) => tables.panel.setValue(property.id, value)}
						onaddoption={(name) => tables.panel.addOption(property.id, name)}
					/>
				</ItemDetail>
			{/each}
		{/if}
	{/snippet}
	<!-- Only a link's own click matters here, and links are reached by keyboard themselves. -->
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="flex flex-1 flex-col" onclick={preview ? onLink : undefined}>
		{#if shown !== null}
			<DocumentBody
				bind:this={body}
				{draft}
				workspace={ws}
				{preview}
				label="Document"
				placeholder="Start writing…"
				fill
				onblur={save}
				onsubmit={done}
			/>
		{/if}
	</div>
</ItemPanel>
