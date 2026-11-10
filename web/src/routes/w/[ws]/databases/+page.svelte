<!--
	A workspace's databases, by title; a row opens its table.
-->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Table2 from '@lucide/svelte/icons/table-2';
	import { untrack } from 'svelte';
	import DatabaseDialog from '$lib/features/databases/DatabaseDialog.svelte';
	import type { Database } from '$lib/protocol/types';
	import { workspaceLoadError } from '$lib/shell/loadError';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { routes } from '$lib/shell/routes';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';

	const { databases } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));

	$effect(() => {
		const id = ws;
		untrack(() => void databases.load(id));
		return () => databases.list.release();
	});

	let creating = $state(false);
	let renaming = $state<Database | null>(null);
	let renameOpen = $state(false);
	let deleting = $state<Database | null>(null);

	async function create(title: string) {
		const database = await databases.create(ws, title);
		void page.goto(routes.database(ws, database.id));
	}
</script>

<PageTitle title="Databases" />

<div class="px-(--page-inset)">
	<PageHeader title="Databases">
		{#snippet actions()}
			<Button variant="secondary" onclick={() => (creating = true)}
				><Plus size={15} strokeWidth={1.75} aria-hidden="true" />New database</Button
			>
		{/snippet}
	</PageHeader>

	{#if databases.list.error && !databases.list.loaded}
		<EmptyState {...workspaceLoadError(databases.list.error, () => databases.list.refetch())} />
	{:else if databases.list.loaded}
		{#if databases.list.value.length === 0}
			<EmptyState
				message="No databases yet"
				actionLabel="New database"
				onaction={() => (creating = true)}
			/>
		{:else}
			<ul>
				{#each databases.list.value as database (database.id)}
					<li class="flex items-center gap-2 border-border not-first:border-t">
						<a
							class="group flex min-w-0 flex-1 items-center gap-3 py-2.5 text-fg hover:no-underline"
							href={routes.database(ws, database.id)}
						>
							<Table2
								class="flex-none text-fg-subtle"
								size={16}
								strokeWidth={1.75}
								aria-hidden="true"
							/>
							<span class="min-w-0 truncate transition-colors group-hover:text-accent"
								>{database.title}</span
							>
						</a>
						<RowMenu
							label="More for {database.title}"
							items={[
								{
									label: 'Rename…',
									onSelect: () => {
										renaming = database;
										renameOpen = true;
									}
								},
								{
									label: 'Delete…',
									danger: true,
									separated: true,
									onSelect: () => {
										deleting = database;
									}
								}
							]}
						/>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
</div>

<DatabaseDialog bind:open={creating} onsubmit={create} />

<DatabaseDialog
	bind:open={renameOpen}
	database={renaming}
	onsubmit={(title) => databases.rename(ws, renaming!.id, title)}
/>

<ConfirmDialog
	subject={deleting}
	onclose={() => (deleting = null)}
	title="Delete “{deleting?.title ?? ''}”?"
	description="All rows in this database and their documents will also be deleted."
	onconfirm={(database) => databases.remove(ws, database.id)}
/>
