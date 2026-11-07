<!--
	Which kinds of resource a workspace shows in the sidebar, and in what
	order: a checkbox per kind of each connector with an account that can
	import, with its connector's icon. The shown kinds come first, in the
	sidebar's order, and reorder by their handles or with Alt+↑/↓; a kind
	shown later goes last. Each box saves as it is ticked; a hidden kind keeps
	its resources.
-->
<script lang="ts">
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import { connectors, type ConnectorKind } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Checkbox from '$lib/ui/Checkbox.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { resources } = pageStores();

	interface Props {
		open: boolean;
		workspaceId: number;
		/** The kinds the workspace shows, in their order. */
		shown: ConnectorKind[];
	}

	let { open = $bindable(), workspaceId, shown }: Props = $props();

	const order = new Reorderable(
		() => shown,
		({ kind }) => kind.id,
		(ids) => resources.reorderShown(workspaceId, ids).catch((e) => toasts.error(e))
	);
	const hidden = $derived(
		connectors.kinds.filter(({ kind }) => !shown.some((s) => s.kind.id === kind.id))
	);

	function toggle(kind_id: number, on: boolean) {
		resources.setShown(workspaceId, kind_id, on).catch((e) => toasts.error(e));
	}

	function onHandleDrag(event: DragEvent, entry: ConnectorKind) {
		order.start(event, entry, entry.kind.plural);
		// The whole row follows the pointer, not only the handle.
		const row = (event.currentTarget as HTMLElement).closest('li')!;
		const { left, top } = row.getBoundingClientRect();
		event.dataTransfer!.setDragImage(row, event.clientX - left, event.clientY - top);
	}
</script>

{#snippet row(entry: ConnectorKind, isShown: boolean, index: number)}
	{@const { connector, kind } = entry}
	<Checkbox
		checked={isShown}
		onchange={(e) => toggle(kind.id, e.currentTarget.checked)}
		aria-keyshortcuts={isShown ? 'Alt+ArrowUp Alt+ArrowDown' : undefined}
		onkeydown={isShown ? (e) => order.key(e, index) : undefined}
	>
		<span class="flex items-center gap-2">
			<IconTile icon={connector.icon} title={connector.title} size={16} />
			{kind.plural}
		</span>
	</Checkbox>
{/snippet}

<Dialog bind:open title="Show in sidebar">
	{#if connectors.connectors.length === 0}
		<EmptyState
			compact
			message="No accounts yet"
			detail="Add one under Accounts to import from it."
		/>
	{:else}
		<ul class="flex flex-col gap-1.5" {...order.list}>
			{#each order.items as entry, index (entry.kind.id)}
				<li
					class={['flex items-center gap-1', order.dragged === entry.kind.id && 'opacity-50']}
					{...order.target(index)}
				>
					<!-- Pointer only: the checkbox takes Alt+↑/↓. -->
					<span
						class="inline-flex cursor-grab text-fg-subtle hover:text-fg"
						draggable="true"
						aria-hidden="true"
						ondragstart={(e) => onHandleDrag(e, entry)}
						ondragend={() => order.end()}
					>
						<GripVertical size={14} strokeWidth={1.75} />
					</span>
					{@render row(entry, true, index)}
				</li>
			{/each}
			{#each hidden as entry (entry.kind.id)}
				<!-- Indented past the handle the shown kinds have. -->
				<li class="flex items-center pl-4.5">
					{@render row(entry, false, -1)}
				</li>
			{/each}
		</ul>
	{/if}
	{#snippet footer()}
		<Button class="border-border-strong!" onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>
