<!--
	The Tasks page's saved views, under its folders: each keeps a list, the tags
	narrowing it and an order under a name. "New view…" saves what the page shows
	now; a view's update button makes it keep that instead.
-->
<script lang="ts">
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import FolderNav from '$lib/features/folders/FolderNav.svelte';
	import { navHeading, navTool } from '$lib/features/folders/nav';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { SavedTaskView } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		views: SavedTaskView[];
		/** The view keeping exactly what the page shows, if one does. */
		current: number | null;
		href: (view: SavedTaskView) => string;
		/** Called as a view's link is followed, so the page can take its order. */
		onpick: (view: SavedTaskView) => void;
		oncreate: (name: string) => Promise<unknown>;
		/** Makes a view keep what the page shows now. */
		onupdate: (view: SavedTaskView) => Promise<unknown>;
		onrename: (view: SavedTaskView, name: string) => Promise<unknown>;
		ondelete: (view: SavedTaskView) => Promise<unknown>;
		onreorder: (viewIds: number[]) => void;
	}

	let { views, current, href, onpick, oncreate, onupdate, onrename, ondelete, onreorder }: Props =
		$props();

	const byId = (id: number) => views.find((v) => v.id === id)!;
	const rows = $derived(views.map((v) => ({ id: v.id, title: v.name })));

	// ---- Naming -------------------------------------------------------------

	let naming = $state<{ mode: 'new' } | { mode: 'rename'; view: SavedTaskView } | null>(null);
	let nameOpen = $state(false);
	let name = $state('');
	let nameError = $state<string | null>(null);
	let busy = $state(false);

	function openNaming(next: NonNullable<typeof naming>) {
		naming = next;
		name = next.mode === 'rename' ? next.view.name : '';
		nameError = null;
		nameOpen = true;
	}

	async function submitName(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (naming === null || trimmed === '') return;
		busy = true;
		try {
			if (naming.mode === 'new') await oncreate(trimmed);
			else await onrename(naming.view, trimmed);
			nameOpen = false;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') nameError = e.message;
			else toasts.error(e);
		} finally {
			busy = false;
		}
	}

	// ---- Deleting -----------------------------------------------------------

	let deleting = $state<SavedTaskView | null>(null);
	let deleteOpen = $state(false);

	async function confirmDelete() {
		if (deleting === null) return;
		busy = true;
		try {
			await ondelete(deleting);
			deleteOpen = false;
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<!-- Labelled even without views yet, so "New view…" reads as part of a group. -->
{#snippet heading()}
	<li class={navHeading}>Views</li>
{/snippet}

<FolderNav
	label="Views"
	newLabel="New view…"
	folders={rows}
	before={heading}
	{current}
	href={(id) => href(byId(id))}
	onpick={(row) => onpick(byId(row.id))}
	onnew={() => openNaming({ mode: 'new' })}
	onrename={(row) => openNaming({ mode: 'rename', view: byId(row.id) })}
	ondelete={(row) => {
		deleting = byId(row.id);
		deleteOpen = true;
	}}
	{onreorder}
>
	{#snippet tools(row)}
		{#if row.id !== current}
			<button
				class={navTool}
				type="button"
				aria-label="Update {row.title} to what’s shown"
				onclick={() => onupdate(byId(row.id)).catch((e) => toasts.error(e))}
			>
				<RefreshCw size={13} strokeWidth={1.75} />
			</button>
		{/if}
	{/snippet}
</FolderNav>

<Dialog
	bind:open={nameOpen}
	title={naming?.mode === 'rename' ? 'Rename view' : 'New view'}
	description={naming?.mode === 'rename'
		? undefined
		: 'Save the list, tags and order shown now under a name.'}
>
	<form id="saved-view-form" onsubmit={submitName}>
		<Field label="Name" for="saved-view-name" error={nameError}>
			<TextInput
				id="saved-view-name"
				bind:value={name}
				autocomplete="off"
				oninput={() => (nameError = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (nameOpen = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="saved-view-form"
			disabled={busy || name.trim() === ''}
		>
			{naming?.mode === 'rename' ? 'Rename' : 'Create'}
		</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={deleteOpen}
	title="Delete “{deleting?.name ?? ''}”?"
	description="Tasks in this view won’t be deleted."
>
	{#snippet footer()}
		<Button onclick={() => (deleteOpen = false)}>Cancel</Button>
		<Button variant="danger-quiet" disabled={busy} onclick={confirmDelete}>Delete</Button>
	{/snippet}
</Dialog>
