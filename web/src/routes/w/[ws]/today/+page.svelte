<script lang="ts">
	import RewindButton from '$lib/features/rewind/RewindButton.svelte';
	import { untrack } from 'svelte';
	import { localDay, longDay, shortTime } from '$lib/format/dates';
	import { send } from '$lib/protocol/api';
	import { editorLinks } from '$lib/shell/embedCards';
	import { routes } from '$lib/shell/routes';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import PageFooter from '$lib/ui/PageFooter.svelte';
	import Editor from '$lib/ui/Editor.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { journal } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	/** Cards for links alone on their lines, and `@` to link this workspace's items. */
	const links = $derived(editorLinks(ws));
	const today = localDay();
	let yesterdayId = $state<number | null>(null);

	$effect(() => {
		const id = ws;
		// Only `ws` should re-run this; `open` reads the store's own state.
		untrack(() => void journal.open(id));
		yesterdayId = null;
		send('listDocuments', {
			workspace_id: id,
			document_type: 'journal',
			journal_from: localDay(1),
			journal_to: today
		})
			.then((docs) => {
				if (id === ws) yesterdayId = docs[0]?.id ?? null;
			})
			.catch(() => undefined);
	});

	$effect(() => {
		if (journal.saveError) toasts.error(journal.saveError);
	});

	function save() {
		journal.flush().catch(() => undefined);
	}

	page.beforeLeave(save);

	$effect(() => {
		const onHide = () => {
			if (document.visibilityState === 'hidden') save();
		};
		document.addEventListener('visibilitychange', onHide);
		return () => {
			document.removeEventListener('visibilitychange', onHide);
			save();
		};
	});

	const status = $derived.by(() => {
		if (journal.saveError) return 'Not saved';
		if (journal.saving || journal.dirty) return 'Saving…';
		if (journal.savedAt === null) return '';
		return `Saved · ${shortTime(journal.savedAt)}`;
	});
</script>

<PageTitle title="Today" />

<!-- Fills the pane, so the editor runs to its foot; the header runs across the sheet like every
     page's, and what follows sits in the conversations' column. -->
<div class="flex min-h-full flex-col px-(--page-inset)">
	<PageHeader title={longDay(today)}>
		{#snippet meta()}
			<span aria-live="polite">{status}</span>
		{/snippet}
		{#snippet actions()}
			{#if journal.today.value}
				<RewindButton
					item={{ kind: 'document', workspace_id: ws, id: journal.today.value.id }}
					title={longDay(today)}
					onopen={save}
					onrestored={(result) => {
						if (result.kind === 'document') journal.restored(result.document);
					}}
				/>
			{/if}
		{/snippet}
	</PageHeader>

	<div class="mx-auto flex w-full max-w-170 flex-1 flex-col pt-6">
		{#if journal.today.error && !journal.today.loaded}
			<EmptyState {...workspaceLoadError(journal.today.error, () => journal.today.refetch())} />
		{:else if journal.today.loaded}
			<Editor
				value={journal.draft}
				language="markdown"
				label="Today’s journal"
				placeholder="Write about today…"
				fill
				size="sm"
				embed={links.embed}
				mentions={links.mentions}
				onchange={(text) => journal.edit(text)}
				onblur={save}
			/>
		{/if}

		<PageFooter label="Journal">
			{#if yesterdayId !== null}
				<a class="text-fg-muted hover:text-accent" href={routes.journalEntry(ws, yesterdayId)}
					>Yesterday</a
				>
			{:else}
				<span class="text-fg-subtle">Yesterday</span>
			{/if}
			<span class="text-fg-subtle" aria-hidden="true">·</span>
			<a class="text-fg-muted hover:text-accent" href={routes.journal(ws)}>All entries</a>
		</PageFooter>
	</div>
</div>
