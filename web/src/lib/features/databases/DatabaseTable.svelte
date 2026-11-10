<!--
	A database as a table: a column per property in its order, a row per entry
	as the daemon lists them. A column's header opens its menu (sort, filter,
	rename, change type, its formula, move, delete); a cell edits in place with
	an editor for its type (`ValueCell`, which a row's panel shares), and a
	formula's cells only read. Each row opens from its leading button or its ⋯
	menu, which also copies a link to it.
-->
<script lang="ts" module>
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Banknote from '@lucide/svelte/icons/banknote';
	import Calendar from '@lucide/svelte/icons/calendar';
	import DecimalsArrowRight from '@lucide/svelte/icons/decimals-arrow-right';
	import Hash from '@lucide/svelte/icons/hash';
	import SquareCheck from '@lucide/svelte/icons/square-check';
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import Type from '@lucide/svelte/icons/type';
	import type { Component } from 'svelte';
	import type { DatabaseDataType } from '$lib/protocol/types';

	/** How narrow a column of each type gets: text needs room to read, a checkbox none. */
	const COLUMN_WIDTHS: Record<DatabaseDataType, string> = {
		text: 'min-w-36',
		integer: 'min-w-24',
		decimal: 'min-w-24',
		currency: 'min-w-24',
		checkbox: 'min-w-20',
		date: 'min-w-28'
	};

	/** The icon a column of each type wears; a formula's wears `SquareFunction` instead. */
	export const TYPE_ICONS: Record<DatabaseDataType, Component> = {
		text: Type,
		integer: Hash,
		decimal: DecimalsArrowRight,
		currency: Banknote,
		checkbox: SquareCheck,
		date: Calendar
	};
</script>

<script lang="ts">
	import FileText from '@lucide/svelte/icons/file-text';
	import Plus from '@lucide/svelte/icons/plus';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { tick } from 'svelte';
	import type {
		DatabaseProperty,
		DatabaseRowEntry,
		DatabaseSort,
		DatabaseValue
	} from '$lib/protocol/types';
	import { copyLinkItems } from '$lib/ui/copyLink';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import ValueCell from './ValueCell.svelte';
	import { cellValue, isNumeric, rowLabel, sortDirections } from './values';

	interface Props {
		properties: DatabaseProperty[];
		rows: DatabaseRowEntry[];
		sort: DatabaseSort | null;
		/** Orders by a property, or as added when null. */
		onsort: (sort: DatabaseSort | null) => void;
		onfilter: (property: DatabaseProperty) => void;
		onrename: (property: DatabaseProperty) => void;
		onretype: (property: DatabaseProperty) => void;
		/** Makes a property a formula, or edits the formula it is. */
		onformula: (property: DatabaseProperty) => void;
		/** Makes a formula a property set by hand again. */
		onsetbyhand: (property: DatabaseProperty) => void;
		/** Moves a property just before `before_id`, or last when it's null. */
		onmove: (property: DatabaseProperty, before_id: number | null) => void;
		ondelete: (property: DatabaseProperty) => void;
		onaddproperty: () => void;
		onopenrow: (row: DatabaseRowEntry) => void;
		/** The workspace the database is in, for a row's link. */
		workspace: number;
		ondeleterow: (row: DatabaseRowEntry) => void;
		/** Sets a row's value, or empties it when null; throws the daemon's refusal. */
		onsetvalue: (
			row: DatabaseRowEntry,
			property: DatabaseProperty,
			value: DatabaseValue | null
		) => Promise<unknown>;
	}

	let {
		properties,
		rows,
		sort,
		onsort,
		onfilter,
		onrename,
		onretype,
		onformula,
		onsetbyhand,
		onmove,
		ondelete,
		onaddproperty,
		onopenrow,
		workspace,
		ondeleterow,
		onsetvalue
	}: Props = $props();

	// ---- Editing a cell -------------------------------------------------------

	/** The cell being edited. */
	let editing = $state<{ row: number; property: number } | null>(null);

	/** Cells that edit as text: every property set by hand but a checkbox. */
	const editable = (p: DatabaseProperty) => p.formula === null && p.data_type !== 'checkbox';

	/** Starts editing the first cell of row `row_id` that edits as text, once it is drawn. */
	export async function editFirst(row_id: number) {
		await tick();
		const property = properties.find(editable);
		if (rows.some((r) => r.id === row_id) && property)
			editing = { row: row_id, property: property.id };
	}

	/** Moves the editor to the editable cell after (or before) the one being edited, in reading order. */
	function moveEditor(back: boolean) {
		if (editing === null) return;
		const cells = rows.flatMap((row) =>
			properties.filter(editable).map((property) => ({ row: row.id, property: property.id }))
		);
		const at = cells.findIndex((c) => c.row === editing!.row && c.property === editing!.property);
		// Past the first or last cell, the editor closes.
		editing = cells[at + (back ? -1 : 1)] ?? null;
	}

	// ---- A column's menu ------------------------------------------------------

	function columnItems(property: DatabaseProperty, index: number): MenuItem[] {
		const [up, down] = sortDirections(property.data_type);
		const sorted = sort?.property_id === property.id ? sort : null;
		return [
			{
				label: 'Sort',
				items: [
					{
						label: up,
						checked: sorted !== null && !sorted.descending,
						onSelect: () => onsort({ property_id: property.id, descending: false })
					},
					{
						label: down,
						checked: sorted?.descending === true,
						onSelect: () => onsort({ property_id: property.id, descending: true })
					},
					{
						label: 'Don’t sort',
						checked: sorted === null,
						separated: true,
						onSelect: () => onsort(null)
					}
				]
			},
			{ label: 'Filter…', onSelect: () => onfilter(property) },
			{ label: 'Rename…', separated: true, onSelect: () => onrename(property) },
			{ label: 'Change type…', onSelect: () => onretype(property) },
			property.formula
				? { label: 'Edit formula…', onSelect: () => onformula(property) }
				: { label: 'Change to formula…', onSelect: () => onformula(property) },
			...(property.formula
				? [{ label: 'Change to set by hand', onSelect: () => onsetbyhand(property) }]
				: []),
			{
				label: 'Move left',
				disabled: index === 0,
				onSelect: () => onmove(property, properties[index - 1].id)
			},
			{
				label: 'Move right',
				disabled: index === properties.length - 1,
				onSelect: () => onmove(property, properties[index + 2]?.id ?? null)
			},
			{ label: 'Delete…', danger: true, separated: true, onSelect: () => ondelete(property) }
		];
	}

	const cellPad = 'px-2.5 py-1.5';
</script>

<div class="overflow-x-auto">
	<table class="min-w-full border-collapse text-sm">
		<thead>
			<tr class="border-b border-border">
				<th class="w-8 p-0"><span class="sr-only">Open</span></th>
				{#each properties as property, index (property.id)}
					{@const Icon = property.formula ? SquareFunction : TYPE_ICONS[property.data_type]}
					{@const sorted = sort?.property_id === property.id ? sort : null}
					<th
						class={['p-0 text-left font-medium', COLUMN_WIDTHS[property.data_type]]}
						aria-sort={sorted ? (sorted.descending ? 'descending' : 'ascending') : undefined}
					>
						<Menu items={columnItems(property, index)} align="start">
							{#snippet trigger(props)}
								<button
									class={[
										'flex w-full cursor-pointer items-center gap-1.5 rounded-sm text-xs whitespace-nowrap text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg',
										cellPad,
										isNumeric(property.data_type) && 'justify-end'
									]}
									type="button"
									{...props}
								>
									<Icon
										class="flex-none text-fg-subtle"
										size={14}
										strokeWidth={1.75}
										aria-hidden="true"
									/>
									<span class="truncate">{property.name}</span>
									{#if property.formula}<span class="sr-only">, formula</span>{/if}
									{#if property.formula?.error}
										<TriangleAlert
											class="flex-none text-warn"
											size={13}
											strokeWidth={2}
											aria-hidden="true"
										/>
										<span class="sr-only">, failed</span>
									{/if}
									{#if sorted}
										{@const Arrow = sorted.descending ? ArrowDown : ArrowUp}
										<Arrow class="flex-none text-fg" size={13} strokeWidth={2} aria-hidden="true" />
									{/if}
								</button>
							{/snippet}
						</Menu>
					</th>
				{/each}
				<!-- The last column stays in view as a wide table scrolls, so a property and a row's
				     menu are always in reach. -->
				<th class="sticky right-0 w-10 bg-bg p-0 text-right">
					<button
						class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
						type="button"
						aria-label="New property"
						onclick={onaddproperty}
					>
						<Plus size={15} strokeWidth={1.75} />
					</button>
				</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as row (row.id)}
				{@const label = rowLabel(row, properties)}
				<tr class="group border-b border-border">
					<td class="p-0 text-center">
						<button
							class={[
								'inline-flex size-7 cursor-pointer items-center justify-center rounded-sm transition-[color,background-color,opacity] group-hover:opacity-100 hover:bg-bg-hover hover:text-fg focus-visible:opacity-100 touch:opacity-100',
								row.document_id === null ? 'text-fg-subtle opacity-0' : 'text-fg-muted'
							]}
							type="button"
							aria-label="Open {label}"
							onclick={() => onopenrow(row)}
						>
							<FileText size={14} strokeWidth={1.75} />
						</button>
					</td>
					{#each properties as property (property.id)}
						{@const value = cellValue(row, property.id)}
						{@const numeric = isNumeric(property.data_type)}
						{@const here = editing?.row === row.id && editing.property === property.id}
						<td class={['p-0 align-middle', numeric && 'text-right tabular-nums']}>
							<ValueCell
								{property}
								{value}
								rowLabel={label}
								look="cell"
								editing={here}
								onedit={() => (editing = { row: row.id, property: property.id })}
								onstop={() => {
									if (here) editing = null;
								}}
								onsave={(next) => onsetvalue(row, property, next)}
								ontab={moveEditor}
							/>
						</td>
					{/each}
					<td class="sticky right-0 w-10 bg-bg p-0 text-right">
						<RowMenu
							label="More for {label}"
							items={[
								{ label: 'Open', onSelect: () => onopenrow(row) },
								...copyLinkItems(() => ({
									link: {
										kind: 'database-row',
										ws: workspace,
										database: row.database_id,
										id: row.id
									},
									title: rowLabel(row, properties)
								})).map((item, i) => ({ ...item, separated: i === 0 })),
								{
									label: 'Delete…',
									danger: true,
									separated: true,
									onSelect: () => ondeleterow(row)
								}
							]}
						/>
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>
