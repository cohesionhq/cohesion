<!--
	A database row's panel (`ItemPanel`), titled with its label: its value for
	each of its database's properties, each changed in place with the table's
	editor for its type (`ValueCell`; a formula's only reads), then its
	document the way a task's panel holds its description: read as Markdown,
	or written in the documents' editor after Edit, saving as it goes. A row
	with nothing written opens in the editor, and gains its document with the
	first save, so opening a row never makes one. Open goes to the database's
	table. It follows the row as it changes anywhere (`databases.row`), and
	closes once the row is deleted.
-->
<script lang="ts">
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import TableProperties from '@lucide/svelte/icons/table-properties';
	import { tick, untrack } from 'svelte';
	import DocumentBody from '$lib/features/documents/DocumentBody.svelte';
	import SavedMark from '$lib/features/documents/SavedMark.svelte';
	import { useAutosave } from '$lib/features/documents/autosave.svelte';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { routes } from '$lib/shell/routes';
	import type { DatabaseRowKey } from '$lib/stores/databases.svelte';
	import { Draft } from '$lib/stores/draft.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import { TYPE_ICONS } from './DatabaseTable.svelte';
	import ValueCell from './ValueCell.svelte';
	import { cellValue, rowLabel } from './values';

	interface Props {
		open?: boolean;
		ws: number;
		database: number;
		row: number;
		/** The row's label as its link resolved, shown until the row loads. */
		label: string;
	}

	let { open = $bindable(false), ws, database, row: rowId, label }: Props = $props();

	const { databases } = pageStores();

	/** Whether the store's row is the one this panel shows. */
	const current = $derived.by(() => {
		const key = databases.row.key;
		return key?.workspace_id === ws && key.database_id === database && key.row_id === rowId;
	});
	const entry = $derived(current ? databases.row.value : null);
	const properties = $derived(current ? databases.rowProperties.value : []);
	const databaseTitle = $derived(current ? databases.rowDatabase.value?.title : undefined);
	const title = $derived(entry && properties.length ? rowLabel(entry, properties) : label);

	// Loads the row each time the panel opens on one, and lets it go once closed.
	$effect(() => {
		const [isOpen, w, d, r] = [open, ws, database, rowId];
		untrack(() => {
			if (isOpen) void databases.openRow(w, d, r);
		});
		return () => {
			if (isOpen) untrack(() => databases.closeRow());
		};
	});

	// A row deleted anywhere closes its panel.
	$effect(() => {
		if (open && current && databases.row.loaded && databases.row.value === null) open = false;
	});

	// ---- Its values -------------------------------------------------------------

	/** The property whose value is being edited. */
	let editing = $state<number | null>(null);

	// ---- Its document -------------------------------------------------------------

	/** The row the draft is of; saves go to it. */
	let subject: DatabaseRowKey | null = null;
	const draft = new Draft(async (content) => {
		const document = await databases.writeRowDocument(subject!, content);
		return document.modified;
	});
	const save = useAutosave(draft);

	let preview = $state(true);
	let body = $state<DocumentBody>();
	/** The row the panel last started on; null once closed. */
	let shown = $state<string | null>(null);
	const document = $derived(current ? databases.rowDocument.value : null);
	/** Whether the row's document (or the lack of one) has loaded. */
	const settled = $derived(
		current &&
			databases.row.loaded &&
			(entry?.document_id == null || databases.rowDocument.value?.id === entry.document_id)
	);

	// Start from the row's saved document each time the panel opens on a row,
	// once the last one's edits are saved; a refetch while it is open lands
	// unless there are unsaved edits. Closing saves.
	$effect(() => {
		const [isOpen, ready, doc] = [open, settled, document];
		const key = `${ws}/${database}/${rowId}`;
		untrack(() => {
			if (!isOpen) {
				if (shown !== null) save();
				shown = null;
			} else if (ready && key !== shown) {
				shown = key;
				void start({ workspace_id: ws, database_id: database, row_id: rowId }, doc?.contents ?? '');
			} else if (ready && doc && doc.contents !== draft.saved) {
				draft.receive(doc.contents, doc.modified);
			}
		});
	});

	async function start(next: DatabaseRowKey, contents: string) {
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
	kind={databaseTitle ?? vocabulary.embedKind('databaseRow')}
	href={routes.database(ws, database)}
	link={{ kind: 'database-row', ws, database, id: rowId }}
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
						onsave={(value) => databases.setRowValue(property.id, value)}
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
				placeholder="Notes, links, anything about this row…"
				fill
				onblur={save}
				onsubmit={done}
			/>
		{/if}
	</div>
</ItemPanel>
