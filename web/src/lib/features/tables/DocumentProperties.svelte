<!--
	A document's values on its own page, under its title, when its folder is
	a table: a row per column, each changed in place with the table's editor
	for its type (`ValueCell`; a formula's only reads), as its row's panel
	shows them. Nothing shows for a document in a plain folder.
-->
<script lang="ts">
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import { untrack } from 'svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import { TYPE_ICONS } from './DocumentTable.svelte';
	import ValueCell from './ValueCell.svelte';
	import { cellValue } from './values';

	interface Props {
		workspace: number;
		/** The document's folder, a table. */
		folder: number;
		document: number;
	}

	let { workspace, folder, document }: Props = $props();

	const { tables } = pageStores();
	const values = tables.page;

	$effect(() => {
		const [w, f, d] = [workspace, folder, document];
		untrack(() => void values.open(w, f, d));
		return () => values.release();
	});

	const current = $derived.by(() => {
		const key = values.row.key;
		return (
			key?.workspace_id === workspace && key.folder_id === folder && key.document_id === document
		);
	});
	const row = $derived(current ? values.row.value : null);
	const properties = $derived(
		current && values.properties.key?.folder_id === folder ? values.properties.value : []
	);

	/** The property whose value is being edited. */
	let editing = $state<number | null>(null);
</script>

{#if row && properties.length}
	<dl
		class="mb-6 grid grid-cols-[max-content_minmax(0,1fr)] items-start gap-x-6 gap-y-1 text-sm"
		aria-label="Values"
	>
		{#each properties as property (property.id)}
			<ItemDetail
				label={property.name}
				icon={property.formula ? SquareFunction : TYPE_ICONS[property.data_type]}
			>
				<ValueCell
					{property}
					value={cellValue(row, property.id)}
					rowLabel={row.title}
					look="detail"
					editing={editing === property.id}
					onedit={() => (editing = property.id)}
					onstop={() => {
						if (editing === property.id) editing = null;
					}}
					onsave={(value) => values.setValue(property.id, value)}
					onaddoption={(name) => values.addOption(property.id, name)}
				/>
			</ItemDetail>
		{/each}
	</dl>
{/if}
