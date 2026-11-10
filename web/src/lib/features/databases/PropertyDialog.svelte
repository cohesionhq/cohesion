<!--
	A property's name, type and formula: a new property takes a name, whether
	its values are set by hand or come from a formula, and its type (a
	formula's, the type it returns); renaming one its name; changing its type
	the type, with a warning that values that don't fit the new type are
	cleared; and its formula the source, which turns a property set by hand
	into a formula after a warning that its values will be deleted. A formula
	starts from an example written with the database's own properties. The
	daemon's refusal shows under its field.
-->
<script lang="ts" module>
	import type { DatabaseDataType, DatabaseProperty } from '$lib/protocol/types';

	/** What the dialog is doing. */
	export type PropertyEdit =
		| { mode: 'new' }
		| { mode: 'rename'; property: DatabaseProperty }
		| { mode: 'type'; property: DatabaseProperty }
		| { mode: 'formula'; property: DatabaseProperty };

	export interface PropertyFields {
		name: string;
		data_type: DatabaseDataType;
		/** The formula's source, or null for values set by hand. */
		formula: string | null;
	}
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import Select from '$lib/ui/Select.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import FormulaField from './FormulaField.svelte';
	import { formulaStarter } from './formulas';

	interface Props {
		open?: boolean;
		edit: PropertyEdit | null;
		/** The database's properties, which a formula's example reads. */
		properties: DatabaseProperty[];
		onsubmit: (fields: PropertyFields) => Promise<unknown>;
	}

	let { open = $bindable(false), edit, properties, onsubmit }: Props = $props();

	const SOURCES = [
		{ value: 'hand', label: 'Set by hand' },
		{ value: 'formula', label: 'Formula' }
	];

	let name = $state('');
	let dataType = $state<string>('text');
	/** Whether a new property's values come from a formula. */
	let values = $state('hand');
	let source = $state('');
	/** The type the example in `source` was written for. */
	let starterType = 'text';
	let error = $state<string | null>(null);
	/** Which field the refusal is of, so it shows under that one. */
	let errorField = $state<'name' | 'type' | 'formula'>('type');
	let busy = $state(false);

	const except = $derived(edit && edit.mode !== 'new' ? edit.property.id : null);
	const starter = (type: string) =>
		formulaStarter(
			type as DatabaseDataType,
			properties,
			untrack(() => except)
		);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			const property = edit && edit.mode !== 'new' ? edit.property : null;
			name = property?.name ?? '';
			dataType = property?.data_type ?? 'text';
			values = 'hand';
			source = property?.formula?.source ?? starter(dataType);
			starterType = dataType;
			error = null;
		});
	});

	const mode = $derived(edit?.mode ?? 'new');
	/** The property the formula is of, when the dialog edits one. */
	const formulaOf = $derived(edit?.mode === 'formula' ? edit.property : null);
	const converting = $derived(formulaOf !== null && formulaOf.formula === null);
	const withFormula = $derived(mode === 'formula' || (mode === 'new' && values === 'formula'));

	const title = $derived.by(() => {
		switch (edit?.mode) {
			case 'rename':
				return 'Rename property';
			case 'type':
				return `Change the type of “${edit.property.name}”?`;
			case 'formula':
				return converting
					? `Change “${edit.property.name}” to a formula?`
					: `Edit formula for “${edit.property.name}”`;
			default:
				return 'New property';
		}
	});
	const unchanged = $derived(
		edit?.mode === 'rename'
			? name.trim() === edit.property.name
			: edit?.mode === 'type'
				? dataType === edit.property.data_type
				: edit?.mode === 'formula' && source === edit.property.formula?.source
	);
	const description = $derived.by(() => {
		// Text reads every value, so only another type clears any.
		if (mode === 'type' && dataType !== 'text')
			return 'Values that don’t fit the new type will be cleared. This can’t be undone.';
		if (converting)
			return `The values set by hand in “${formulaOf?.name}” will be deleted and replaced by what the formula returns. This can’t be undone.`;
		return undefined;
	});

	const submitLabel = $derived(
		mode === 'rename'
			? 'Rename'
			: mode === 'type'
				? 'Change type'
				: converting
					? 'Change to formula'
					: mode === 'formula'
						? 'Save'
						: 'Create'
	);

	/** A new formula's example follows its type until it is edited. */
	function retype(next: string) {
		if (source === starter(starterType)) source = starter(next);
		starterType = next;
		error = null;
	}

	const blank = $derived(
		(mode === 'new' || mode === 'rename') && name.trim() === ''
			? true
			: withFormula && source.trim() === ''
	);

	async function submit(event?: SubmitEvent) {
		event?.preventDefault();
		if (blank || busy) return;
		busy = true;
		try {
			await onsubmit({
				name: name.trim(),
				data_type: dataType as DatabaseDataType,
				formula: withFormula ? source : null
			});
			open = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') {
				error = failure.message;
				errorField =
					mode === 'rename' || (mode === 'new' && failure.field === 'name')
						? 'name'
						: mode === 'formula' || failure.field === 'formula'
							? 'formula'
							: 'type';
			} else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open {title} {description}>
	<form class="flex flex-col gap-4" id="property-form" onsubmit={submit}>
		{#if mode === 'new' || mode === 'rename'}
			<Field label="Name" for="property-name" error={errorField === 'name' ? error : null}>
				<TextInput
					id="property-name"
					bind:value={name}
					autocomplete="off"
					oninput={() => (error = null)}
				/>
			</Field>
		{/if}
		{#if mode === 'new'}
			<Field label="Values" for="property-values">
				<Select
					id="property-values"
					bind:value={values}
					options={SOURCES}
					onchange={() => (error = null)}
				/>
			</Field>
		{/if}
		{#if mode === 'new' || mode === 'type'}
			<Field label="Type" for="property-type" error={errorField === 'type' ? error : null}>
				<Select
					id="property-type"
					bind:value={dataType}
					options={options(vocabulary.value.database_data_types)}
					onchange={retype}
				/>
			</Field>
		{/if}
		{#if withFormula}
			<FormulaField
				{source}
				type={dataType as DatabaseDataType}
				error={errorField === 'formula' ? error : null}
				failure={source === formulaOf?.formula?.source ? formulaOf.formula.error : null}
				onchange={(text) => {
					source = text;
					error = null;
				}}
				onsubmit={() => void submit()}
			/>
		{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="property-form"
			disabled={busy || unchanged || blank}
		>
			{submitLabel}
		</Button>
	{/snippet}
</Dialog>
