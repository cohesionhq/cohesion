<!--
	The Tasks header's Views menu: the lists across folders (All, Done, Cancelled), "Save view…" to
	keep what the page shows now (the list, the tags narrowing it and an order) under a name, and
	"Update view" to make a saved view keep that instead. The saved views themselves are listed in the
	sidebar, under Tasks, where they are renamed, deleted and put in order.
-->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { SavedTaskView } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { secondaryClass } from '$lib/ui/Button.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		views: SavedTaskView[];
		/** The list shown when no saved view keeps what the page shows. */
		list: 'inbox' | 'all' | 'done' | 'cancelled' | number | null;
		/** The view keeping exactly what the page shows, if one does. */
		current: number | null;
		onlist: (list: 'all' | 'done' | 'cancelled') => void;
		oncreate: (name: string) => Promise<unknown>;
		/** Makes a view keep what the page shows now. */
		onupdate: (view: SavedTaskView) => Promise<unknown>;
	}

	let { views, list, current, onlist, oncreate, onupdate }: Props = $props();

	const LISTS = [
		['all', 'All'],
		['done', 'Done'],
		['cancelled', 'Cancelled']
	] as const;

	const items = $derived<MenuItem[]>([
		...LISTS.map(([value, label]) => ({
			label,
			checked: current === null && list === value,
			onSelect: () => onlist(value)
		})),
		{ label: 'Save view…', separated: true, onSelect: openNaming },
		...(views.length > 0
			? [
					{
						label: 'Update view to what’s shown',
						items: views.map((v) => ({
							id: v.id,
							label: v.name,
							disabled: v.id === current,
							onSelect: () => onupdate(v).catch((e) => toasts.error(e))
						}))
					}
				]
			: [])
	]);

	// ---- Naming -------------------------------------------------------------

	let nameOpen = $state(false);
	let name = $state('');
	let nameError = $state<string | null>(null);
	let busy = $state(false);

	function openNaming() {
		name = '';
		nameError = null;
		nameOpen = true;
	}

	async function submitName(event: SubmitEvent) {
		event.preventDefault();
		const trimmed = name.trim();
		if (trimmed === '') return;
		busy = true;
		try {
			await oncreate(trimmed);
			nameOpen = false;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') nameError = e.message;
			else toasts.error(e);
		} finally {
			busy = false;
		}
	}
</script>

<Menu {items} align="start">
	{#snippet trigger(props)}
		<button
			class={[
				'inline-flex h-7.5 flex-none cursor-pointer items-center gap-1.5 rounded-sm border px-2.5 font-medium transition-colors',
				secondaryClass
			]}
			type="button"
			{...props}
		>
			Views
			<ChevronDown size={14} strokeWidth={1.75} aria-hidden="true" />
		</button>
	{/snippet}
</Menu>

<Dialog
	bind:open={nameOpen}
	title="New view"
	description="Save the list, tags and order shown now under a name."
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
			Create
		</Button>
	{/snippet}
</Dialog>
