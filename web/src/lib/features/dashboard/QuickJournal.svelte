<!--
	Quick journal: a dialog to jot a note and add it to the end of today's
	entry in one go, without leaving the Dashboard. ⌘/Ctrl+Enter adds it.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Textarea from '$lib/ui/Textarea.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	let { ws, open = $bindable(false) }: { ws: number; open?: boolean } = $props();

	const { journal } = pageStores();

	let text = $state('');
	let adding = $state(false);
	let error = $state<string | null>(null);
	let field = $state<HTMLTextAreaElement>();

	// Focus the note as the dialog opens.
	$effect(() => {
		if (open) field?.focus();
	});

	async function add() {
		const note = text.trim();
		if (note === '' || adding) return;
		adding = true;
		error = null;
		try {
			await journal.append(ws, note);
			open = false;
			toasts.show('Added to today’s journal.', 'info');
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			adding = false;
		}
	}
</script>

<Dialog
	bind:open
	title="Quick journal"
	description="Your note will be added to the end of today’s entry."
	size="wide"
	onclosed={() => ((text = ''), (error = null))}
>
	<Textarea
		bind:value={text}
		bind:ref={field}
		rows={6}
		aria-label="Note"
		placeholder="Write about today…"
		onkeydown={(event) => {
			if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
				event.preventDefault();
				void add();
			}
		}}
	/>
	{#if error}
		<p class="mt-2 text-sm text-danger" role="alert">{error}</p>
	{/if}
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" disabled={text.trim() === '' || adding} onclick={add}>
			Add to journal
		</Button>
	{/snippet}
</Dialog>
