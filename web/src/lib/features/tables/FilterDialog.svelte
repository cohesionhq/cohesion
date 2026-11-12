<!--
	Adds a filter: a column (the title or a property), one of the conditions
	its type allows, and the value or text the condition needs, typed for the
	column, or a choice's option picked from its list. The daemon applies it.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { DocumentFolderProperty, PropertyValue, TableFilter } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import DayField from '$lib/features/tasks/DayField.svelte';
	import {
		type ColumnInfo,
		columnKey,
		conditionChoices,
		isNumeric,
		parseInput,
		tableColumns
	} from './values';

	interface Props {
		open?: boolean;
		properties: DocumentFolderProperty[];
		/** The column the dialog starts on; the title when null. */
		column: ColumnInfo | null;
		onadd: (filter: TableFilter) => void;
	}

	let { open = $bindable(false), properties, column, onadd }: Props = $props();

	const columns = $derived(tableColumns(properties));

	let columnId = $state('');
	let choiceId = $state('');
	let text = $state('');
	let error = $state<string | null>(null);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			columnId = columnKey(column?.column ?? { kind: 'title' });
			choiceId = '';
			text = '';
			error = null;
		});
	});

	const chosen = $derived(columns.find((c) => columnKey(c.column) === columnId) ?? null);
	const choices = $derived(
		chosen ? conditionChoices(chosen.data_type, chosen.column.kind === 'title') : []
	);
	const choice = $derived(choices.find((c) => c.id === choiceId) ?? choices[0] ?? null);
	/** A choice's value is one of its options, by id; the first until another is picked. */
	const picking = $derived(chosen?.data_type === 'choice' && choice?.takes === 'value');
	const optionId = $derived(
		chosen?.options.some((o) => String(o.id) === text) ? text : String(chosen?.options[0]?.id ?? '')
	);

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (chosen === null || choice === null) return;
		let value: PropertyValue | null = null;
		if (choice.takes === 'value') {
			const parsed = parseInput(chosen.data_type, picking ? optionId : text);
			if ('error' in parsed) {
				error = parsed.error;
				return;
			}
			if (parsed.value === null) {
				error = 'Enter a value.';
				return;
			}
			value = parsed.value;
		}
		onadd({ column: chosen.column, condition: choice.condition(value, text) });
		open = false;
	}
</script>

<Dialog bind:open title="Add filter">
	<form class="flex flex-col gap-4" id="filter-form" onsubmit={submit}>
		<Field label="Column" for="filter-column">
			<Select
				id="filter-column"
				bind:value={columnId}
				options={columns.map((c) => ({ value: columnKey(c.column), label: c.name }))}
				onchange={() => {
					choiceId = '';
					text = '';
					error = null;
				}}
			/>
		</Field>
		{#if chosen}
			<Field label="Condition" for="filter-condition">
				<Select
					id="filter-condition"
					value={choice?.id ?? ''}
					options={choices.map((c) => ({ value: c.id, label: c.label }))}
					onchange={(next) => {
						choiceId = next;
						error = null;
					}}
				/>
			</Field>
			{#if picking}
				<Field label="Option" for="filter-value" {error}>
					<Select
						id="filter-value"
						value={optionId}
						options={chosen.options.map((o) => ({ value: String(o.id), label: o.name }))}
						onchange={(next) => {
							text = next;
							error = null;
						}}
					/>
				</Field>
			{:else if choice && choice.takes !== 'nothing'}
				<Field label="Value" for="filter-value" {error}>
					{#if chosen.data_type === 'date' && choice.takes === 'value'}
						<DayField
							id="filter-value"
							bind:value={
								() => text,
								(day) => {
									text = day;
									error = null;
								}
							}
						/>
					{:else}
						<TextInput
							id="filter-value"
							bind:value={text}
							inputmode={chosen.data_type === 'integer'
								? 'numeric'
								: isNumeric(chosen.data_type)
									? 'decimal'
									: undefined}
							autocomplete="off"
							oninput={() => (error = null)}
						/>
					{/if}
				</Field>
			{/if}
		{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="filter-form"
			disabled={chosen === null ||
				(choice?.takes === 'text' && text === '') ||
				(picking && optionId === '')}
		>
			Add filter
		</Button>
	{/snippet}
</Dialog>
