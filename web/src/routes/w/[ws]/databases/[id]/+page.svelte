<!--
	One database as a table. The filters and sort are the page's own: they go
	to the daemon with every listing (`listDatabaseRows`), which filters and
	sorts, and they last while the page's half keeps the database open. A row
	opens in the panel over the page (`DatabaseRowPanel`), as `?row=` opens
	one when the page loads.
-->
<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Funnel from '@lucide/svelte/icons/funnel';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { untrack } from 'svelte';
	import DatabaseDialog from '$lib/features/databases/DatabaseDialog.svelte';
	import DatabaseTable from '$lib/features/databases/DatabaseTable.svelte';
	import FilterDialog from '$lib/features/databases/FilterDialog.svelte';
	import PropertyDialog, {
		type PropertyEdit,
		type PropertyFields
	} from '$lib/features/databases/PropertyDialog.svelte';
	import { filterText, rowLabel, sortDirections } from '$lib/features/databases/values';
	import { pillClass } from '$lib/features/tags/FilterPill.svelte';
	import type {
		DatabaseFilter,
		DatabaseProperty,
		DatabaseRowEntry,
		DatabaseSort
	} from '$lib/protocol/types';
	import { linkedItem } from '$lib/shell/linkedItem.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { routes } from '$lib/shell/routes';
	import { NO_QUERY } from '$lib/stores/databases.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { copyLinkItems } from '$lib/ui/copyLink';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import MoreActions from '$lib/ui/MoreActions.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	const { databases } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const id = $derived(Number(page.params.id));
	const database = $derived(databases.database.value);
	const properties = $derived(databases.properties.value);
	const rows = $derived(databases.rows.value);
	const query = $derived(databases.query);

	$effect(() => {
		const [w, d] = [ws, id];
		untrack(() => void databases.open(w, d));
	});
	$effect(() => () => {
		databases.database.release();
		databases.properties.release();
		databases.rows.release();
	});

	function run(action: Promise<unknown>) {
		action.catch((error) => toasts.error(error));
	}

	const propertyOf = (property_id: number) => properties.find((p) => p.id === property_id);

	// ---- Filters and sort -------------------------------------------------------

	let filtering = $state(false);
	let filterFrom = $state<DatabaseProperty | null>(null);

	function addFilter(filter: DatabaseFilter) {
		void databases.setQuery({ ...query, filters: [...query.filters, filter] });
	}

	function removeFilter(index: number) {
		void databases.setQuery({ ...query, filters: query.filters.toSpliced(index, 1) });
	}

	function setSort(sort: DatabaseSort | null) {
		void databases.setQuery({ ...query, sort });
	}

	const sortProperty = $derived(query.sort ? propertyOf(query.sort.property_id) : undefined);

	// ---- Properties ---------------------------------------------------------------

	let propertyEdit = $state<PropertyEdit | null>(null);
	let propertyOpen = $state(false);

	function editProperty(edit: PropertyEdit) {
		propertyEdit = edit;
		propertyOpen = true;
	}

	function submitProperty({ name, data_type, formula }: PropertyFields) {
		const edit = propertyEdit;
		if (edit?.mode === 'rename') return databases.renameProperty(edit.property.id, name);
		if (edit?.mode === 'type') return databases.setPropertyType(edit.property.id, data_type);
		if (edit?.mode === 'formula') return databases.setPropertyFormula(edit.property.id, formula);
		return databases.createProperty(name, data_type, formula);
	}

	let deletingProperty = $state<DatabaseProperty | null>(null);

	// ---- Rows ---------------------------------------------------------------------

	let table = $state<DatabaseTable>();

	async function newRow() {
		try {
			const row = await databases.createRow();
			await table?.editFirst(row.id);
		} catch (error) {
			toasts.error(error);
		}
	}

	/** Opens a row in the panel over the page. */
	function openRow(row_id: number): Promise<void> {
		return linkedItem.show({ kind: 'database-row', ws, database: id, id: row_id });
	}

	// A link to a row lands here with `?row=`: open it over the table, then
	// drop the parameter, once the navigation that brought it has settled.
	$effect(() => {
		const param = page.url.searchParams.get('row');
		if (param === null) return;
		untrack(() => void reveal(Number(param)));
	});

	async function reveal(row_id: number) {
		await openRow(row_id);
		const url = new URL(page.url);
		url.searchParams.delete('row');
		await page.goto(url.pathname + url.search, { replaceState: true });
	}

	let deletingRow = $state<DatabaseRowEntry | null>(null);

	// ---- The database -------------------------------------------------------------

	let renaming = $state(false);
	let deleteOpen = $state(false);

	async function remove() {
		await databases.remove(ws, id);
		void page.goto(routes.databases(ws), { replaceState: true });
	}

	const failing = $derived(properties.filter((p) => p.formula?.error));
</script>

<PageTitle title={database ? `${database.title} · Databases` : 'Databases'} />

<div class="px-(--page-inset)">
	<ResourceState
		resource={databases.database}
		missing="Database not found"
		missingAction="Back to databases"
		onmissing={() => page.goto(routes.databases(ws), { replaceState: true })}
	>
		{#snippet loaded(db)}
			<PageHeader title={db.title} back={{ href: routes.databases(ws), label: 'Databases' }}>
				{#snippet actions()}
					{#if properties.length > 0}
						<Button
							variant="secondary"
							onclick={() => {
								filterFrom = null;
								filtering = true;
							}}><Funnel size={14} strokeWidth={1.75} aria-hidden="true" />Add filter…</Button
						>
						<Button variant="secondary" onclick={newRow}
							><Plus size={15} strokeWidth={1.75} aria-hidden="true" />New row</Button
						>
					{/if}
					<MoreActions
						items={[
							{ label: 'Rename…', onSelect: () => (renaming = true) },
							...copyLinkItems(() => ({
								link: { kind: 'database', ws, id },
								title: database?.title ?? ''
							})),
							{
								label: 'Delete…',
								danger: true,
								separated: true,
								onSelect: () => (deleteOpen = true)
							}
						]}
					/>
				{/snippet}
			</PageHeader>

			<div class="pb-8">
				{#if query.filters.length > 0 || (query.sort && sortProperty)}
					<ul class="mb-3 flex flex-wrap items-center gap-1.5" aria-label="Filters and sort">
						{#if query.sort && sortProperty}
							{@const Arrow = query.sort.descending ? ArrowDown : ArrowUp}
							{@const text = `${sortProperty.name}: ${sortDirections(sortProperty.data_type)[query.sort.descending ? 1 : 0]}`}
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
						{#each query.filters as filter, index (index)}
							{@const property = propertyOf(filter.property_id)}
							{#if property}
								{@const text = filterText(filter, property)}
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

				{#if databases.properties.loaded}
					{#if properties.length === 0}
						<EmptyState
							message="No properties yet"
							detail="Add a property for each kind of value the rows hold."
							actionLabel="New property"
							onaction={() => editProperty({ mode: 'new' })}
						/>
					{:else}
						<DatabaseTable
							bind:this={table}
							{properties}
							{rows}
							sort={query.sort}
							onsort={setSort}
							onfilter={(property) => {
								filterFrom = property;
								filtering = true;
							}}
							onrename={(property) => editProperty({ mode: 'rename', property })}
							onretype={(property) => editProperty({ mode: 'type', property })}
							onformula={(property) => editProperty({ mode: 'formula', property })}
							onsetbyhand={(property) => run(databases.setPropertyFormula(property.id, null))}
							onmove={(property, before) => run(databases.moveProperty(property.id, before))}
							ondelete={(property) => {
								deletingProperty = property;
							}}
							onaddproperty={() => editProperty({ mode: 'new' })}
							onopenrow={(row) => void openRow(row.id)}
							workspace={ws}
							ondeleterow={(row) => {
								deletingRow = row;
							}}
							onsetvalue={(row, property, value) => databases.setValue(row.id, property.id, value)}
						/>
						{#if databases.rows.error}
							<EmptyState
								message={databases.rows.error.message}
								actionLabel="Remove filters"
								onaction={() => void databases.setQuery({ ...NO_QUERY, sort: query.sort })}
								compact
							/>
						{:else if databases.rows.loaded && rows.length === 0}
							{#if query.filters.length > 0}
								<EmptyState
									message="No matches"
									detail="Remove a filter to see more rows."
									compact
								/>
							{:else}
								<EmptyState message="No rows yet" actionLabel="New row" onaction={newRow} compact />
							{/if}
						{/if}
					{/if}
				{/if}
			</div>
		{/snippet}
	</ResourceState>
</div>

<PropertyDialog
	bind:open={propertyOpen}
	edit={propertyEdit}
	{properties}
	onsubmit={submitProperty}
/>

<FilterDialog bind:open={filtering} {properties} property={filterFrom} onadd={addFilter} />

<ConfirmDialog
	subject={deletingProperty}
	onclose={() => (deletingProperty = null)}
	title="Delete “{deletingProperty?.name ?? ''}”?"
	description="Every row’s value for this property will also be deleted."
	onconfirm={(property) => databases.removeProperty(property.id)}
/>

<ConfirmDialog
	subject={deletingRow}
	onclose={() => (deletingRow = null)}
	title="Delete “{deletingRow ? rowLabel(deletingRow, properties) : ''}”?"
	description={deletingRow?.document_id != null
		? 'The row’s document will also be deleted.'
		: 'This can’t be undone.'}
	onconfirm={(row) => databases.removeRow(row.id)}
/>

<DatabaseDialog
	bind:open={renaming}
	{database}
	onsubmit={(title) => databases.rename(ws, id, title)}
/>

<ConfirmDialog
	bind:open={deleteOpen}
	title="Delete “{database?.title ?? ''}”?"
	description="All rows in this database and their documents will also be deleted."
	onconfirm={remove}
/>
