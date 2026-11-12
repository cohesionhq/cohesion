<!--
	The Documents page: the Inbox, every
	document (All, leaving out the rows of tables) or one folder. A folder
	that is a table shows as one (`DocumentTable`), its filters and sort the
	page's own: they go to the daemon with every listing (`listTableRows`),
	which filters and sorts, and last while the half keeps the folder open. A
	row opens in the panel over the page.
-->
<script lang="ts">
	import SearchField from '$lib/ui/SearchField.svelte';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import FileText from '@lucide/svelte/icons/file-text';
	import Funnel from '@lucide/svelte/icons/funnel';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { untrack } from 'svelte';
	import { localMidnight, nowSeconds, relativeTime } from '$lib/format/dates';
	import { SEARCH_DEBOUNCE_MS } from '$lib/format/search';
	import type { DocumentSummary } from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { routes } from '$lib/shell/routes';
	import { SERVER_FILTER_THRESHOLD, excerpt, groupByRecency } from '$lib/stores/documents.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { list } from '$lib/ui/list';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { DOCUMENT_DRAG } from '$lib/features/documents/drag';
	import {
		type DocumentView,
		folderPath,
		listedIn,
		tableIds
	} from '$lib/features/documents/folderTree';
	import DocumentFolderSwitcher from '$lib/features/documents/DocumentFolderSwitcher.svelte';
	import FolderHeading, { type Crumb } from '$lib/features/folders/FolderHeading.svelte';
	import DocumentTable from '$lib/features/tables/DocumentTable.svelte';
	import FilterDialog from '$lib/features/tables/FilterDialog.svelte';
	import PropertyDialog, {
		type OptionChanges,
		type PropertyEdit,
		type PropertyFields
	} from '$lib/features/tables/PropertyDialog.svelte';
	import {
		type ColumnInfo,
		columnInfo,
		filterText,
		sortDirections
	} from '$lib/features/tables/values';
	import { pillClass } from '$lib/features/tags/FilterPill.svelte';
	import type {
		DocumentFolderProperty,
		TableFilter,
		TableRow,
		TableSort
	} from '$lib/protocol/types';
	import { folderDialogs } from '$lib/shell/folderDialogs.svelte';
	import { linkedItem } from '$lib/shell/linkedItem.svelte';
	import { NO_QUERY } from '$lib/stores/tables.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { copyLinkItems } from '$lib/ui/copyLink';
	import MoreActions from '$lib/ui/MoreActions.svelte';
	import { asksForNew, asksForNewFolder } from '$lib/shell/shortcuts.svelte';

	const { documents, tables } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	let query = $state('');
	let creating = $state(false);
	/** Server-side matches for `query` when the list is too long to filter here. */
	let found = $state<DocumentSummary[] | null>(null);

	$effect(() => {
		const id = ws;
		untrack(() => {
			void documents.list.load(id);
			void documents.folders.load(id);
		});
		query = '';
	});
	$effect(() => () => documents.folders.release());

	// `?folder=inbox` or a folder's id; every document without it.
	const folderParam = $derived(page.url.searchParams.get('folder'));
	const view = $derived.by((): DocumentView => {
		if (folderParam === 'inbox') return 'inbox';
		const id = Number(folderParam);
		return folderParam !== null && Number.isInteger(id) ? id : 'all';
	});
	function viewHref(next: DocumentView): string {
		const base = routes.documents(ws);
		return next === 'all' ? base : `${base}?folder=${next}`;
	}
	const folder = $derived(
		typeof view === 'number' ? (documents.folders.value.find((f) => f.id === view) ?? null) : null
	);
	const folderGone = $derived(
		typeof view === 'number' && documents.folders.loaded && folder === null
	);
	const viewName = $derived(
		view === 'all' ? 'Documents' : view === 'inbox' ? 'Inbox' : (folder?.title ?? 'Folder')
	);
	/** The way down to what's shown: the page's All, then the folders holding the one shown. */
	const crumbs = $derived.by((): Crumb[] => {
		if (view === 'all') return [];
		const holding =
			typeof view === 'number' ? folderPath(documents.folders.value, view).slice(0, -1) : [];
		return [
			{ label: 'Documents', href: viewHref('all') },
			...holding.map((f) => ({ label: f.title, href: viewHref(f.id) }))
		];
	});
	let switching = $state(false);

	/** The folder new documents go in from here: the one shown, or the Inbox. */
	const newIn = $derived(typeof view === 'number' ? view : null);
	const folderTitles = $derived(new Map(documents.folders.value.map((f) => [f.id, f.title])));
	const tableFolders = $derived(tableIds(documents.folders.value));
	const searching = $derived(query.trim() !== '');
	const inView = (d: DocumentSummary) => listedIn(d, view, tableFolders, searching);

	const remote = $derived(documents.list.value.length > SERVER_FILTER_THRESHOLD);

	$effect(() => {
		const q = query.trim();
		const id = ws;
		found = null;
		if (!remote || q === '') return;
		const timer = setTimeout(() => {
			documents
				.search(id, q)
				.then((docs) => {
					if (q === query.trim() && id === ws) found = docs;
				})
				.catch((error) => toasts.error(error));
		}, SEARCH_DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (q === '') return documents.list.value.filter(inView);
		if (remote) return (found ?? []).filter(inView);
		return documents.list.value.filter((d) => inView(d) && d.title.toLowerCase().includes(q));
	});

	const now = nowSeconds();
	const groups = $derived(groupByRecency(shown, localMidnight()));

	async function create() {
		creating = true;
		try {
			const doc = await documents.create(ws, undefined, undefined, newIn);
			// A table's new row is named where it shows; any other document opens.
			if (isTable) {
				tables.took(doc);
				await table?.editTitle(doc.id);
			} else void page.goto(routes.document(ws, doc.id));
		} catch (error) {
			toasts.error(error);
		} finally {
			creating = false;
		}
	}

	function run(action: Promise<unknown>) {
		action.catch((error) => toasts.error(error));
	}

	// ---- A folder that is a table ---------------------------------------------

	const isTable = $derived(folder?.representation === 'table');

	$effect(() => {
		if (!isTable || typeof view !== 'number') return;
		const [w, f] = [ws, view];
		untrack(() => void tables.open(w, f));
		return () => tables.close();
	});

	/** The shown folder's properties: none until it is a table. */
	const properties = $derived(
		isTable && tables.properties.key?.folder_id === view ? tables.properties.value : []
	);
	const rows = $derived(isTable && tables.rows.key?.folder_id === view ? tables.rows.value : []);
	const tableQuery = $derived(tables.query);
	const sortColumn = $derived(
		tableQuery.sort ? columnInfo(tableQuery.sort.column, properties) : undefined
	);
	const failing = $derived(properties.filter((p) => p.formula?.error));

	let table = $state<DocumentTable>();

	let filtering = $state(false);
	let filterFrom = $state<ColumnInfo | null>(null);

	function openFilter(column: ColumnInfo | null) {
		filterFrom = column;
		filtering = true;
	}

	function addFilter(filter: TableFilter) {
		void tables.setQuery({ ...tableQuery, filters: [...tableQuery.filters, filter] });
	}

	function removeFilter(index: number) {
		void tables.setQuery({ ...tableQuery, filters: tableQuery.filters.toSpliced(index, 1) });
	}

	function setSort(sort: TableSort | null) {
		void tables.setQuery({ ...tableQuery, sort });
	}

	let propertyEdit = $state<PropertyEdit | null>(null);
	let propertyOpen = $state(false);

	function editProperty(edit: PropertyEdit) {
		propertyEdit = edit;
		propertyOpen = true;
	}

	async function submitProperty({ name, data_type, formula, options }: PropertyFields) {
		const edit = propertyEdit;
		if (edit?.mode === 'rename') return tables.renameProperty(edit.property.id, name);
		if (edit?.mode === 'type') return tables.setPropertyType(edit.property.id, data_type);
		if (edit?.mode === 'formula') return tables.setPropertyFormula(edit.property.id, formula);
		if (folder === null) return;
		const folder_id = folder.id;
		await tables.createProperty({ workspace_id: ws, folder_id }, name, data_type, formula, options);
		// A new column makes the folder show as a table.
		documents.markRepresentation(folder_id, 'table');
	}

	const optionChanges: OptionChanges = {
		add: (property_id, name) => tables.addOption(property_id, name),
		rename: (property_id, option_id, name) => tables.renameOption(property_id, option_id, name),
		color: (property_id, option_id, color) => tables.setOptionColor(property_id, option_id, color),
		move: (property_id, option_id, before_id) =>
			tables.moveOption(property_id, option_id, before_id),
		remove: (property_id, option_id) => tables.removeOption(property_id, option_id),
		usage: (property_id, option_id) => tables.optionUsage(property_id, option_id)
	};

	let deletingProperty = $state<DocumentFolderProperty | null>(null);
	/** Whether the column being deleted is the table's last. */
	const deletingLast = $derived(
		deletingProperty !== null && properties.every((p) => p.id === deletingProperty!.id)
	);

	async function removeProperty(property: DocumentFolderProperty) {
		const folder_id = tables.properties.key!.folder_id;
		await tables.removeProperty(property.id);
		// Deleting the last shows the folder as a list again.
		if (tables.properties.value.length === 0)
			await documents.setRepresentation(ws, folder_id, 'list');
	}

	let deletingRow = $state<TableRow | null>(null);

	async function retitle(row: TableRow, title: string) {
		tables.took(await documents.renameDocument(ws, row.document_id, title));
	}

	/** ⌘N makes a document and ⇧⌘N a folder, unless a dialog is open. */
	function onWindowKey(event: KeyboardEvent) {
		if (event.target instanceof HTMLElement && event.target.closest('[role="dialog"]')) return;
		if (asksForNewFolder(event)) {
			event.preventDefault();
			folderDialogs.request({ kind: 'documents', mode: 'new', parent: null });
		} else if (asksForNew(event) && !creating && !folderGone) {
			event.preventDefault();
			void create();
		}
	}
</script>

<svelte:window onkeydown={onWindowKey} />

<PageTitle title={view === 'all' ? 'Documents' : `${viewName} · Documents`} />

<div class="px-(--page-inset)">
	<PageHeader>
		{#snippet heading()}
			<FolderHeading {crumbs} title={viewName} bind:open={switching}>
				<DocumentFolderSwitcher
					folders={documents.folders.value}
					current={view}
					href={viewHref}
					onpick={() => (switching = false)}
					onmove={(f, parent_id, before_id) =>
						run(documents.moveFolder(ws, f.id, parent_id, before_id))}
					onpin={(f, pinned) => run(documents.setFolderPinned(ws, f.id, pinned))}
				/>
			</FolderHeading>
		{/snippet}
		{#snippet actions()}
			{#if isTable}
				<Button variant="secondary" onclick={() => openFilter(null)}
					><Funnel size={14} strokeWidth={1.75} aria-hidden="true" />Add filter…</Button
				>
			{/if}
			<Button variant="secondary" disabled={creating || folderGone} onclick={create}
				><Plus size={15} strokeWidth={1.75} aria-hidden="true" />New document</Button
			>
			{#if folder}
				{@const shown = folder}
				<MoreActions
					items={[
						{ label: 'Add column…', onSelect: () => editProperty({ mode: 'new' }) },
						...copyLinkItems(() => ({
							link: { kind: 'document-folder', ws, id: shown.id },
							title: shown.title
						})).map((item, i) => ({ ...item, separated: i === 0 }))
					]}
				/>
			{/if}
		{/snippet}
	</PageHeader>

	<div class="min-w-0 flex-1">
		{#if folderGone}
			<EmptyState
				message="Folder not found"
				actionLabel="Back to documents"
				onaction={() => page.goto(viewHref('all'), { replaceState: true })}
			/>
		{:else if isTable}
			{@render tableView()}
		{:else if documents.list.error && !documents.list.loaded}
			<EmptyState {...workspaceLoadError(documents.list.error, () => documents.list.refetch())} />
		{:else if documents.list.loaded}
			{#if documents.list.value.length === 0}
				<EmptyState message="No documents yet" actionLabel="New document" onaction={create} />
			{:else if view !== 'all' && !documents.list.value.some(inView)}
				<EmptyState message="No documents" actionLabel="New document" onaction={create} />
			{:else}
				<div class="mb-5">
					<SearchField
						bind:value={query}
						placeholder="Filter by title"
						label="Filter documents by title"
					/>
				</div>

				{#if groups.length === 0}
					{#if !searching}
						<!-- All, when every document is a table's row. -->
						<EmptyState message="No documents" actionLabel="New document" onaction={create} />
					{:else if !remote || found !== null}
						<p class="text-fg-muted">No documents match “{query.trim()}”.</p>
					{/if}
				{:else}
					{#each groups as group (group.key)}
						<section class="[&+section]:mt-6" aria-labelledby="group-{group.key}">
							<!-- Headed as the Dashboard's sections are: the day, a count, a hairline. -->
							<h2 class={list.heading} id="group-{group.key}">
								{group.label}<span class={list.count}>{group.items.length}</span>
							</h2>
							<ul>
								{#each group.items as doc (doc.id)}
									{@const line = excerpt(doc.opening, 140, doc.title)}
									<!-- Two lines, as an article's row: the title, then where the text starts. -->
									<li class="border-border not-first:border-t">
										<a
											class="group flex items-start gap-3 py-2.5 text-fg hover:no-underline"
											href={routes.document(ws, doc.id)}
											ondragstart={(e) => {
												e.dataTransfer?.setData(DOCUMENT_DRAG, String(doc.id));
												if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
											}}
										>
											<FileText
												class="mt-0.5 flex-none text-fg-subtle"
												size={16}
												strokeWidth={1.75}
												aria-hidden="true"
											/>
											<span class="flex min-w-0 flex-1 flex-col gap-0.5">
												<span class="flex items-baseline justify-between gap-4">
													<span class="min-w-0 truncate transition-colors group-hover:text-accent"
														>{doc.title}</span
													>
													<span class="flex flex-none gap-3 text-xs text-fg-subtle tabular-nums">
														{#if view === 'all'}
															<!-- Across folders, each row names where it's filed. -->
															<span
																>{doc.folder_id === null
																	? 'Inbox'
																	: (folderTitles.get(doc.folder_id) ?? '')}</span
															>
														{/if}
														<span>{relativeTime(doc.modified, now)}</span>
													</span>
												</span>
												<span
													class={[
														'max-w-170 truncate text-xs',
														line ? 'text-fg-subtle' : 'text-fg-subtle/70'
													]}>{line || 'Empty'}</span
												>
											</span>
										</a>
									</li>
								{/each}
							</ul>
						</section>
					{/each}
				{/if}
			{/if}
		{/if}
	</div>
</div>

<PropertyDialog
	bind:open={propertyOpen}
	edit={propertyEdit}
	{properties}
	onsubmit={submitProperty}
	{optionChanges}
/>

<FilterDialog bind:open={filtering} {properties} column={filterFrom} onadd={addFilter} />

<ConfirmDialog
	subject={deletingProperty}
	onclose={() => (deletingProperty = null)}
	title="Delete “{deletingProperty?.name ?? ''}”?"
	description={deletingLast
		? `Every document’s value for this column will also be deleted. “${folder?.title ?? ''}” will be a plain folder again, listing its documents.`
		: 'Every document’s value for this column will also be deleted.'}
	onconfirm={removeProperty}
/>

<ConfirmDialog
	subject={deletingRow}
	onclose={() => (deletingRow = null)}
	title="Delete “{deletingRow?.title ?? ''}”?"
	description="This can’t be undone."
	onconfirm={async (row) => tables.took(await documents.deleteDocument(ws, row.document_id))}
/>

{#snippet tableView()}
	<div class="pb-8">
		{#if tableQuery.filters.length > 0 || (tableQuery.sort && sortColumn)}
			<ul class="mb-3 flex flex-wrap items-center gap-1.5" aria-label="Filters and sort">
				{#if tableQuery.sort && sortColumn}
					{@const Arrow = tableQuery.sort.descending ? ArrowDown : ArrowUp}
					{@const text = `${sortColumn.name}: ${sortDirections(sortColumn.data_type)[tableQuery.sort.descending ? 1 : 0]}`}
					<li class={pillClass('with')}>
						<Arrow size={12} strokeWidth={2} aria-hidden="true" />
						<span class="sr-only">Sorted by</span>
						{text}
						<button
							class="-mr-1 inline-flex cursor-pointer rounded-full p-0.5 hover:bg-accent/15"
							type="button"
							aria-label="Remove sort"
							onclick={() => setSort(null)}><X size={12} strokeWidth={2} /></button
						>
					</li>
				{/if}
				{#each tableQuery.filters as filter, index (index)}
					{@const column = columnInfo(filter.column, properties)}
					{#if column}
						{@const text = filterText(filter, column)}
						<li class={pillClass('with')}>
							{text}
							<button
								class="-mr-1 inline-flex cursor-pointer rounded-full p-0.5 hover:bg-accent/15"
								type="button"
								aria-label="Remove filter: {text}"
								onclick={() => removeFilter(index)}><X size={12} strokeWidth={2} /></button
							>
						</li>
					{/if}
				{/each}
			</ul>
		{/if}

		{#each failing as property (property.id)}
			<p class="mb-3 text-xs text-warn">
				The formula for “{property.name}” failed on some rows. {property.formula?.error}
			</p>
		{/each}

		{#if tables.properties.loaded && tables.properties.key?.folder_id === view}
			<DocumentTable
				bind:this={table}
				{properties}
				{rows}
				sort={tableQuery.sort}
				onsort={setSort}
				onfilter={openFilter}
				onrename={(property) => editProperty({ mode: 'rename', property })}
				onretype={(property) => editProperty({ mode: 'type', property })}
				onformula={(property) => editProperty({ mode: 'formula', property })}
				onsetbyhand={(property) => run(tables.setPropertyFormula(property.id, null))}
				onoptions={(property) => editProperty({ mode: 'options', property })}
				onaddoption={(property, name) => tables.addOption(property.id, name)}
				onmove={(property, before) => run(tables.moveProperty(property.id, before))}
				ondelete={(property) => (deletingProperty = property)}
				onaddproperty={() => editProperty({ mode: 'new' })}
				onnewrow={create}
				onopenrow={(row) => void linkedItem.show({ kind: 'document', ws, id: row.document_id })}
				workspace={ws}
				ondeleterow={(row) => (deletingRow = row)}
				onretitle={retitle}
				onsetvalue={(row, property, value) => tables.setValue(row.document_id, property.id, value)}
			/>
			{#if tables.rows.error}
				<EmptyState
					message={tables.rows.error.message}
					actionLabel="Remove filters"
					onaction={() => void tables.setQuery({ ...NO_QUERY, sort: tableQuery.sort })}
					compact
				/>
			{:else if tables.rows.loaded && rows.length === 0 && tableQuery.filters.length > 0}
				<EmptyState message="No matches" detail="Remove a filter to see more rows." compact />
			{/if}
		{/if}
	</div>
{/snippet}
