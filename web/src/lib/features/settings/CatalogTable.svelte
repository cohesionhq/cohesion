<!-- The model catalog as a dense, sortable table. Scrolls sideways on narrow screens. -->
<script lang="ts">
	import { perMillion } from '$lib/format/money';
	import { tokenCount } from '$lib/format/usage';
	import type { CatalogModel } from '$lib/protocol/types';
	import { filterCatalog, type CatalogSort } from '$lib/stores/settings/intelligence.svelte';

	interface Props {
		models: CatalogModel[];
		query: string;
	}

	let { models, query }: Props = $props();

	let sort = $state<CatalogSort>('provider');
	let descending = $state(false);

	const rows = $derived(filterCatalog(models, query, sort, descending));

	const columns: { key: CatalogSort; label: string; numeric: boolean }[] = [
		{ key: 'name', label: 'Model', numeric: false },
		{ key: 'provider', label: 'Provider', numeric: false },
		{ key: 'context', label: 'Context', numeric: true },
		{ key: 'input', label: 'Input / 1M', numeric: true },
		{ key: 'output', label: 'Output / 1M', numeric: true }
	];

	function sortBy(key: CatalogSort) {
		if (sort === key) descending = !descending;
		else {
			sort = key;
			descending = false;
		}
	}

	// Every header and body cell; the first and last sit flush with the table's edges.
	const cell = 'border-b border-border px-2 py-1 align-baseline first:pl-0 last:pr-0';
	const numeric = 'text-right whitespace-nowrap';

	function tokens(count: number | null): string {
		return count === null ? '—' : tokenCount(count);
	}
</script>

<div class="overflow-x-auto overscroll-y-auto border-t border-border">
	<table class="w-full min-w-140 border-collapse text-xs tabular-nums">
		<thead>
			<tr>
				{#each columns as column (column.key)}
					<th
						class={[cell, 'font-medium whitespace-nowrap', column.numeric ? numeric : 'text-left']}
						aria-sort={sort === column.key ? (descending ? 'descending' : 'ascending') : undefined}
					>
						<button
							class="cursor-pointer text-fg-muted hover:text-fg"
							type="button"
							onclick={() => sortBy(column.key)}
						>
							{column.label}
							{#if sort === column.key}<span aria-hidden="true">{descending ? '↓' : '↑'}</span>{/if}
						</button>
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each rows as model (model.id)}
				<tr>
					<td class={[cell, 'text-left']}>
						<span class="block">{model.model_name}</span>
						<span class="block font-mono text-2xs wrap-anywhere text-fg-subtle"
							>{model.model_identifier}</span
						>
					</td>
					<td class={[cell, 'text-left whitespace-nowrap text-fg-muted']}>{model.provider_name}</td>
					<td class={[cell, numeric]}>{tokens(model.context_size)}</td>
					<td class={[cell, numeric]}>{perMillion(model.input_token_cost)}</td>
					<td class={[cell, numeric]}>{perMillion(model.output_token_cost)}</td>
				</tr>
			{:else}
				<tr>
					<td
						class="border-b border-border py-3 text-left align-baseline text-fg-muted"
						colspan={columns.length}
					>
						{query.trim() === ''
							? 'No models yet. Refresh the catalog to fetch them.'
							: 'No matches.'}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>
