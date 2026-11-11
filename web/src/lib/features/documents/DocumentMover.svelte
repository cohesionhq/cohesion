<!--
	Moves a document to a folder or the Inbox, the one way every page moves
	one (a drop on the folder tree, "Move to"). Leaving a table deletes the
	document's values, so when it holds any set by hand
	(`DocumentsStore.moveLosesValues`) it asks first; otherwise it moves
	straight away. Going into a table asks nothing: the document arrives with
	no values.
-->
<script lang="ts">
	import type { Document } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	type Moved = Pick<Document, 'workspace_id' | 'id' | 'folder_id' | 'title'>;

	interface Props {
		/** Runs with each document once it has moved. */
		onmoved?: (doc: Document) => void;
	}

	let { onmoved }: Props = $props();

	const { documents } = pageStores();

	let asking = $state<{ doc: Moved; to: number | null } | null>(null);

	/** Files `doc` in `to` (the Inbox when null), asking first when it would lose values. */
	export async function move(doc: Moved, to: number | null): Promise<void> {
		if (doc.folder_id === to) return;
		try {
			if (await documents.moveLosesValues(doc, to)) asking = { doc, to };
			else await go(doc, to);
		} catch (error) {
			toasts.error(error);
		}
	}

	async function go(doc: Moved, to: number | null) {
		const moved = await documents.move(doc, to);
		onmoved?.(moved);
	}

	const titleOf = (id: number | null) =>
		documents.folders.value.find((f) => f.id === id)?.title ?? '';
	const destination = $derived(
		asking === null ? '' : asking.to === null ? 'the Inbox' : `“${titleOf(asking.to)}”`
	);
</script>

<ConfirmDialog
	subject={asking}
	onclose={() => (asking = null)}
	title="Move “{asking?.doc.title ?? ''}” to {destination}?"
	description="The values “{asking?.doc.title ?? ''}” has in “{titleOf(
		asking?.doc.folder_id ?? null
	)}” will be removed."
	confirmLabel="Move"
	onconfirm={({ doc, to }) => go(doc, to)}
/>
