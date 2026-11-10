<script lang="ts">
	import { untrack } from 'svelte';
	import DocumentBody from '$lib/features/documents/DocumentBody.svelte';
	import DocumentMenu from '$lib/features/documents/DocumentMenu.svelte';
	import { useAutosave } from '$lib/features/documents/autosave.svelte';
	import { localDay, longDay } from '$lib/format/dates';
	import { routes } from '$lib/shell/routes';
	import { neighbours } from '$lib/stores/documents.svelte';
	import PageFooter from '$lib/ui/PageFooter.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { documents } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const id = $derived(Number(page.params.id));
	const today = localDay();
	const entry = $derived(documents.current.value);

	let preview = $state(false);

	$effect(() => {
		const [w, d] = [ws, id];
		untrack(() => {
			void documents.open(w, d);
			void documents.archive.load(w);
		});
		preview = false;
	});

	// Today's entry lives on Today.
	$effect(() => {
		if (entry?.id === id && entry.journal_date === today) {
			void page.goto(routes.today(ws), { replaceState: true });
		}
	});

	const save = useAutosave(documents.text);
	const near = $derived(neighbours(documents.archive.value, id));

	async function remove() {
		try {
			await documents.remove();
			void page.goto(routes.journal(ws), { replaceState: true });
		} catch (error) {
			toasts.error(error);
		}
	}
</script>

<PageTitle title={`${entry ? `${longDay(entry.journal_date!)} · Journal` : 'Journal'}`} />

<!-- Fills the pane, so the editor runs to its foot; the header runs across the sheet like every
     page's, and the entry sits in the conversations' column. -->
<div class="flex min-h-full flex-col px-(--page-inset)">
	<ResourceState
		resource={documents.current}
		missing="Entry not found"
		missingAction="Back to the journal"
		onmissing={() => page.goto(routes.journal(ws), { replaceState: true })}
	>
		{#snippet loaded(entry)}
			{#if entry.journal_date !== today}
				<PageHeader
					title={longDay(entry.journal_date!)}
					back={{ href: routes.journal(ws), label: 'Journal' }}
				>
					{#snippet actions()}
						<DocumentMenu
							bind:preview
							subject="the entry for {longDay(entry.journal_date!)}"
							draft={documents.text}
							forward={{
								kind: 'journal',
								id,
								workspace_id: ws,
								title: entry.title
							}}
							ondelete={remove}
							rewind={{ kind: 'document', workspace_id: ws, id }}
							title={longDay(entry.journal_date!)}
							onrestored={(result) => {
								if (result.kind === 'document') documents.restored(result.document);
							}}
						/>
					{/snippet}
				</PageHeader>

				<div class="mx-auto flex w-full max-w-170 flex-1 flex-col pt-6">
					<DocumentBody
						draft={documents.text}
						workspace={ws}
						{preview}
						label="Journal entry"
						placeholder="Write about this day…"
						fill
						size="sm"
						onblur={save}
					/>

					<PageFooter label="Journal">
						{#if near.prev}
							<a
								class="text-fg-muted hover:text-accent"
								href={routes.journalEntry(ws, near.prev.id)}>Previous entry</a
							>
						{/if}
						{#if near.prev && near.next}<span class="text-fg-subtle" aria-hidden="true">·</span
							>{/if}
						{#if near.next}
							<a
								class="text-fg-muted hover:text-accent"
								href={near.next.journal_date === today
									? routes.today(ws)
									: routes.journalEntry(ws, near.next.id)}>Next entry</a
							>
						{/if}
					</PageFooter>
				</div>
			{/if}
		{/snippet}
	</ResourceState>
</div>
