<!-- The editor pages' Rewind symbol and overflow menu (save time, preview toggle, copy as Markdown, forward to an
     agent and a confirmed delete), with a check beside it once the draft is saved. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import { type ForwardedItem, forwarding } from '$lib/features/agents/forward.svelte';
	import RewindButton from '$lib/features/rewind/RewindButton.svelte';
	import type { Restored, RewindItem } from '$lib/stores/rewind.svelte';
	import { shortTime } from '$lib/format/dates';
	import type { Draft } from '$lib/stores/draft.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Menu from '$lib/ui/Menu.svelte';
	import { copyToClipboard } from '$lib/ui/toasts.svelte';
	import SavedMark from './SavedMark.svelte';

	interface Props {
		preview: boolean;
		/** What the confirmation asks to delete: “Groceries”, or the entry for a day. */
		subject: string;
		draft: Draft;
		/** The page's document as "Forward" sends it to an agent. */
		forward: ForwardedItem;
		ondelete: () => Promise<void>;
		/** The page's document, as Rewind opens it, and what its panel is titled. */
		rewind: RewindItem;
		title: string;
		onrestored: (restored: Restored) => void;
	}

	let {
		preview = $bindable(),
		subject,
		draft,
		forward,
		ondelete,
		rewind,
		title,
		onrestored
	}: Props = $props();

	let confirming = $state(false);
	let busy = $state(false);

	async function confirm() {
		busy = true;
		try {
			await ondelete();
			confirming = false;
		} finally {
			busy = false;
		}
	}
</script>

<div class="flex items-center gap-1.5">
	<SavedMark {draft} />
	<RewindButton
		item={rewind}
		{title}
		onopen={() => draft.flush().catch(() => undefined)}
		{onrestored}
	/>
	<Menu
		items={[
			{ label: preview ? 'Edit' : 'Preview', onSelect: () => (preview = !preview) },
			{
				label: 'Copy as Markdown',
				onSelect: () => copyToClipboard(draft.draft, 'Copied as Markdown.')
			},
			forwarding.submenu(() => {
				// The agent reads the saved copy, so save what's typed first.
				draft.flush().catch(() => undefined);
				return forward;
			}),
			{ label: 'Delete…', danger: true, onSelect: () => (confirming = true) }
		]}
	>
		{#snippet trigger(props)}
			<button
				class="inline-flex size-7.5 cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
				type="button"
				aria-label="More actions"
				{...props}
			>
				<Ellipsis size={16} strokeWidth={1.75} />
			</button>
		{/snippet}
		{#snippet header()}
			{draft.status(shortTime)}
		{/snippet}
	</Menu>
</div>

<Dialog bind:open={confirming} title="Delete {subject}?" description="This can’t be undone.">
	{#snippet footer()}
		<Button onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="danger-quiet" disabled={busy} onclick={confirm}>Delete</Button>
	{/snippet}
</Dialog>
