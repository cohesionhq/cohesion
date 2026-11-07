<script lang="ts">
	import { untrack } from 'svelte';
	import DocumentBody from '$lib/features/documents/DocumentBody.svelte';
	import DocumentMenu from '$lib/features/documents/DocumentMenu.svelte';
	import { useAutosave } from '$lib/features/documents/autosave.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { routes } from '$lib/shell/routes';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { documents } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const id = $derived(Number(page.params.id));
	const doc = $derived(documents.current.value);

	let preview = $state(false);
	let title = $state('');
	let editingTitle = $state(false);
	let titleError = $state<string | null>(null);

	$effect(() => {
		const [w, d] = [ws, id];
		untrack(() => void documents.open(w, d));
		preview = false;
		titleError = null;
	});

	// A journal opened by id (from search, say) belongs in the journal.
	$effect(() => {
		if (doc?.id === id && doc.document_type === 'journal') {
			void page.goto(routes.journalEntry(ws, id), { replaceState: true });
		}
	});

	// Follow the server's title unless the heading is being edited.
	$effect(() => {
		const current = doc?.title ?? '';
		if (!untrack(() => editingTitle)) title = current;
	});

	const save = useAutosave(documents.text);

	async function commitTitle() {
		editingTitle = false;
		const old = doc?.title ?? '';
		const next = title.trim();
		if (doc === null || next === old) {
			title = old;
			return;
		}
		try {
			await documents.rename(next);
			titleError = null;
		} catch (error) {
			const e = toProtocolError(error);
			title = old;
			if (e.code === 'validationFailed') titleError = e.message;
			else toasts.error(e);
		}
	}

	function onTitleKey(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			(event.currentTarget as HTMLInputElement).blur();
		} else if (event.key === 'Escape') {
			title = doc?.title ?? '';
			(event.currentTarget as HTMLInputElement).blur();
		}
	}

	async function remove() {
		try {
			await documents.remove();
			void page.goto(routes.documents(ws), { replaceState: true });
		} catch (error) {
			toasts.error(error);
		}
	}
</script>

<PageTitle title={`${doc ? `${doc.title} · Documents` : 'Documents'}`} />

<!-- In the conversations' column, so lines stay a readable length. -->
<div class="mx-auto w-full max-w-[calc(var(--spacing)*190+2*var(--page-inset))] px-(--page-inset)">
	<ResourceState
		resource={documents.current}
		missing="Document not found"
		missingAction="Back to documents"
		onmissing={() => page.goto(routes.documents(ws), { replaceState: true })}
	>
		{#snippet loaded()}
			<PageHeader back={{ href: routes.documents(ws), label: 'Documents' }}>
				{#snippet heading()}
					<!-- A heading, not a box: focus shows as an accent underline. -->
					<div
						class="min-w-[12ch] flex-1 [&_input]:h-auto [&_input]:rounded-none [&_input]:border-x-0 [&_input]:border-t-0 [&_input]:border-b [&_input]:border-b-transparent [&_input]:bg-transparent [&_input]:p-0 [&_input]:text-lg [&_input]:leading-[1.25] [&_input]:font-semibold [&_input]:text-ellipsis [&_input:focus]:border-b-accent [&_input:focus]:shadow-none"
					>
						<TextInput
							bind:value={title}
							aria-label="Title"
							aria-invalid={titleError !== null}
							aria-describedby={titleError ? 'title-error' : undefined}
							autocomplete="off"
							spellcheck="false"
							onfocus={() => (editingTitle = true)}
							onblur={commitTitle}
							onkeydown={onTitleKey}
							oninput={() => (titleError = null)}
						/>
					</div>
				{/snippet}
				{#snippet actions()}
					<DocumentMenu
						bind:preview
						subject="“{title.trim() || 'Untitled'}”"
						draft={documents.text}
						forward={{
							kind: 'document',
							id,
							workspace_id: ws,
							title: title.trim() || 'Untitled'
						}}
						ondelete={remove}
						rewind={{ kind: 'document', workspace_id: ws, id }}
						title={title.trim() || 'Untitled'}
						onrestored={(result) => {
							if (result.kind === 'document') documents.restored(result.document);
						}}
					/>
				{/snippet}
			</PageHeader>
			{#if titleError}<p class="-mt-4 mb-4 text-xs text-danger" id="title-error">
					{titleError}
				</p>{/if}

			<DocumentBody
				draft={documents.text}
				{preview}
				label="Document"
				placeholder="Start writing…"
				onblur={save}
			/>
		{/snippet}
	</ResourceState>
</div>
