<!--
	A folder that is a table: a row per document as the daemon lists them, its
	title first (renaming the document), then a column per property in its
	order. A column's header opens its menu (sort and filter; a property's also
	rename, change type, its formula (never a choice's), a choice's options, move, delete); a cell edits in place with
	an editor for its type (`ValueCell`, which a row's panel shares), and a
	formula's cells only read. Each row opens from its leading button or its ⋯
	menu, which also copies a link to it, and drags onto the folder tree like
	a document in a list. A "New" line at the foot adds a document.
-->
<script lang="ts" module>
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Banknote from '@lucide/svelte/icons/banknote';
	import Calendar from '@lucide/svelte/icons/calendar';
	import CircleChevronDown from '@lucide/svelte/icons/circle-chevron-down';
	import DecimalsArrowRight from '@lucide/svelte/icons/decimals-arrow-right';
	import Hash from '@lucide/svelte/icons/hash';
	import SquareCheck from '@lucide/svelte/icons/square-check';
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import Type from '@lucide/svelte/icons/type';
	import type { Component } from 'svelte';
	import type { DocumentFolderProperty, PropertyDataType } from '$lib/protocol/types';

	/** How narrow a column of each type gets: text needs room to read, a checkbox none. */
	const COLUMN_WIDTHS: Record<PropertyDataType, string> = {
		text: 'min-w-36',
		integer: 'min-w-24',
		decimal: 'min-w-24',
		currency: 'min-w-24',
		checkbox: 'min-w-20',
		date: 'min-w-28',
		choice: 'min-w-28'
	};

	/** The icon a column of each type wears; a formula's wears `SquareFunction` instead. */
	export const TYPE_ICONS: Record<PropertyDataType, Component> = {
		text: Type,
		integer: Hash,
		decimal: DecimalsArrowRight,
		currency: Banknote,
		checkbox: SquareCheck,
		date: Calendar,
		choice: CircleChevronDown
	};

	/** The title as `ValueCell` edits it: text, set by hand, under the id no property has. */
	const TITLE_PROPERTY: DocumentFolderProperty = {
		id: 0,
		folder_id: 0,
		name: 'Title',
		data_type: 'text',
		sort_order: 0,
		formula: null
	};
</script>

<script lang="ts">
	import FileText from '@lucide/svelte/icons/file-text';
	import Plus from '@lucide/svelte/icons/plus';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { tick } from 'svelte';
	import { DOCUMENT_DRAG } from '$lib/features/documents/DocumentFolderTree.svelte';
	import type { PropertyValue, TableRow, TableSort } from '$lib/protocol/types';
	import { copyLinkItems } from '$lib/ui/copyLink';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import ValueCell from './ValueCell.svelte';
	import {
		type ColumnInfo,
		TITLE_COLUMN,
		cellValue,
		isNumeric,
		propertyColumn,
		sameColumn,
		sortDirections
	} from './values';

	interface Props {
		properties: DocumentFolderProperty[];
		rows: TableRow[];
		sort: TableSort | null;
		/** Orders by a column, or as the documents were created when null. */
		onsort: (sort: TableSort | null) => void;
		onfilter: (column: ColumnInfo) => void;
		onrename: (property: DocumentFolderProperty) => void;
		onretype: (property: DocumentFolderProperty) => void;
		/** Makes a property a formula, or edits the formula it is. */
		onformula: (property: DocumentFolderProperty) => void;
		/** Makes a formula a property set by hand again. */
		onsetbyhand: (property: DocumentFolderProperty) => void;
		/** Edits a choice's options. */
		onoptions: (property: DocumentFolderProperty) => void;
		/** Moves a property just before `before_id`, or last when it's null. */
		onmove: (property: DocumentFolderProperty, before_id: number | null) => void;
		ondelete: (property: DocumentFolderProperty) => void;
		onaddproperty: () => void;
		/** Adds a document to the table. */
		onnewrow: () => void;
		onopenrow: (row: TableRow) => void;
		/** The workspace the table is in, for a row's link. */
		workspace: number;
		ondeleterow: (row: TableRow) => void;
		/** Renames a row's document; throws the daemon's refusal. */
		onretitle: (row: TableRow, title: string) => Promise<unknown>;
		/** Sets a row's value, or empties it when null; throws the daemon's refusal. */
		onsetvalue: (
			row: TableRow,
			property: DocumentFolderProperty,
			value: PropertyValue | null
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
		onoptions,
		onmove,
		ondelete,
		onaddproperty,
		onnewrow,
		onopenrow,
		workspace,
		ondeleterow,
		onretitle,
		onsetvalue
	}: Props = $props();

	// ---- Editing a cell -------------------------------------------------------

	/** The cell being edited: a row's document and a property, the title's being 0. */
	let editing = $state<{ row: number; property: number } | null>(null);

	/** Cells that edit as text: the title, and every property set by hand but a checkbox or a choice. */
	const editable = (p: DocumentFolderProperty) =>
		p.formula === null && p.data_type !== 'checkbox' && p.data_type !== 'choice';
	const editableColumns = $derived([TITLE_PROPERTY, ...properties.filter(editable)]);

	/** Starts editing the title of the row of document `document_id`, once it is drawn. */
	export async function editTitle(document_id: number) {
		await tick();
		if (rows.some((r) => r.document_id === document_id))
			editing = { row: document_id, property: TITLE_PROPERTY.id };
	}

	/** Moves the editor to the editable cell after (or before) the one being edited, in reading order. */
	function moveEditor(back: boolean) {
		if (editing === null) return;
		const cells = rows.flatMap((row) =>
			editableColumns.map((property) => ({ row: row.document_id, property: property.id }))
		);
		const at = cells.findIndex((c) => c.row === editing!.row && c.property === editing!.property);
		// Past the first or last cell, the editor closes.
		editing = cells[at + (back ? -1 : 1)] ?? null;
	}

	// ---- A column's menu ------------------------------------------------------

	function sortItems(column: ColumnInfo): MenuItem {
		const [up, down] = sortDirections(column.data_type);
		const sorted = sort && sameColumn(sort.column, column.column) ? sort : null;
		return {
			label: 'Sort',
			items: [
				{
					label: up,
					checked: sorted !== null && !sorted.descending,
					onSelect: () => onsort({ column: column.column, descending: false })
				},
				{
					label: down,
					checked: sorted?.descending === true,
					onSelect: () => onsort({ column: column.column, descending: true })
				},
				{
					label: 'Don’t sort',
					checked: sorted === null,
					separated: true,
					onSelect: () => onsort(null)
				}
			]
		};
	}

	function columnItems(property: DocumentFolderProperty, index: number): MenuItem[] {
		const column = propertyColumn(property);
		return [
			sortItems(column),
			{ label: 'Filter…', onSelect: () => onfilter(column) },
			{ label: 'Rename…', separated: true, onSelect: () => onrename(property) },
			{ label: 'Change type…', onSelect: () => onretype(property) },
			// A choice is never a formula; its options are what it edits.
			property.data_type === 'choice'
				? { label: 'Edit options…', onSelect: () => onoptions(property) }
				: property.formula
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

	/** Lets a row be dragged onto the folder tree, as a document in a list is. */
	function dragRow(event: DragEvent, row: TableRow) {
		if (!event.dataTransfer) return;
		event.dataTransfer.setData(DOCUMENT_DRAG, String(row.document_id));
		// Firefox only starts a drag that carries data; a drop elsewhere gets the title.
		event.dataTransfer.setData('text/plain', row.title);
		event.dataTransfer.effectAllowed = 'move';
	}

	const cellPad = 'px-2.5 py-1.5';
	const headerButton =
		'flex w-full cursor-pointer items-center gap-1.5 rounded-sm text-xs whitespace-nowrap text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg data-[state=open]:bg-bg-hover data-[state=open]:text-fg';
</script>

{#snippet sortArrow(column: ColumnInfo)}
	{#if sort && sameColumn(sort.column, column.column)}
		{@const Arrow = sort.descending ? ArrowDown : ArrowUp}
		<Arrow class="flex-none text-fg" size={13} strokeWidth={2} aria-hidden="true" />
	{/if}
{/snippet}

<div class="overflow-x-auto">
	<table class="min-w-full border-collapse text-sm">
		<thead>
			<tr class="border-b border-border">
				<th class="w-8 p-0"><span class="sr-only">Open</span></th>
				<th
					class="min-w-48 p-0 text-left font-medium"
					aria-sort={sort?.column.kind === 'title'
						? sort.descending
							? 'descending'
							: 'ascending'
						: undefined}
				>
					<Menu
						items={[
							sortItems(TITLE_COLUMN),
							{ label: 'Filter…', onSelect: () => onfilter(TITLE_COLUMN) }
						]}
						align="start"
					>
						{#snippet trigger(props)}
							<button class={[headerButton, cellPad]} type="button" {...props}>
								<FileText
									class="flex-none text-fg-subtle"
									size={14}
									strokeWidth={1.75}
									aria-hidden="true"
								/>
								<span class="truncate">{TITLE_COLUMN.name}</span>
								{@render sortArrow(TITLE_COLUMN)}
							</button>
						{/snippet}
					</Menu>
				</th>
				{#each properties as property, index (property.id)}
					{@const Icon = property.formula ? SquareFunction : TYPE_ICONS[property.data_type]}
					{@const column = propertyColumn(property)}
					{@const sorted = sort && sameColumn(sort.column, column.column) ? sort : null}
					<th
						class={['p-0 text-left font-medium', COLUMN_WIDTHS[property.data_type]]}
						aria-sort={sorted ? (sorted.descending ? 'descending' : 'ascending') : undefined}
					>
						<Menu items={columnItems(property, index)} align="start">
							{#snippet trigger(props)}
								<button
									class={[headerButton, cellPad, isNumeric(property.data_type) && 'justify-end']}
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
									{@render sortArrow(column)}
								</button>
							{/snippet}
						</Menu>
					</th>
				{/each}
				<!-- The last column stays in view as a wide table scrolls, so adding a column and a row's
				     menu are always in reach. -->
				<th class="sticky right-0 w-10 bg-bg p-0 text-right">
					<button
						class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
						type="button"
						aria-label="Add column"
						onclick={onaddproperty}
					>
						<Plus size={15} strokeWidth={1.75} />
					</button>
				</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as row (row.document_id)}
				{@const label = row.title}
				<tr
					class="group border-b border-border"
					draggable={editing?.row !== row.document_id}
					ondragstart={(e) => dragRow(e, row)}
				>
					<td class="p-0 text-center">
						<button
							class="inline-flex size-7 cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
							type="button"
							aria-label="Open {label}"
							onclick={() => onopenrow(row)}
						>
							<FileText size={14} strokeWidth={1.75} />
						</button>
					</td>
					<td class="p-0 align-middle font-medium">
						<ValueCell
							property={TITLE_PROPERTY}
							value={{ type: 'text', value: row.title }}
							rowLabel={label}
							look="cell"
							editing={editing?.row === row.document_id && editing.property === TITLE_PROPERTY.id}
							onedit={() => (editing = { row: row.document_id, property: TITLE_PROPERTY.id })}
							onstop={() => {
								if (editing?.row === row.document_id && editing.property === TITLE_PROPERTY.id)
									editing = null;
							}}
							onsave={(next) => onretitle(row, next?.type === 'text' ? next.value : '')}
							ontab={moveEditor}
						/>
					</td>
					{#each properties as property (property.id)}
						{@const value = cellValue(row, property.id)}
						{@const numeric = isNumeric(property.data_type)}
						{@const here = editing?.row === row.document_id && editing.property === property.id}
						<td class={['p-0 align-middle', numeric && 'text-right tabular-nums']}>
							<ValueCell
								{property}
								{value}
								rowLabel={label}
								look="cell"
								editing={here}
								onedit={() => (editing = { row: row.document_id, property: property.id })}
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
									link: { kind: 'document', ws: workspace, id: row.document_id },
									title: row.title
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
			<tr>
				<td class="p-0" colspan={properties.length + 3}>
					<button
						class="flex w-full cursor-pointer items-center gap-1.5 rounded-sm px-1.5 py-1.5 text-left text-fg-subtle transition-colors hover:bg-bg-hover hover:text-fg"
						type="button"
						aria-label="New document"
						onclick={onnewrow}
					>
						<Plus size={14} strokeWidth={1.75} aria-hidden="true" />New
					</button>
				</td>
			</tr>
		</tbody>
	</table>
</div>
