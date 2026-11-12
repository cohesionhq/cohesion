<!--
	A column's name, type and formula (a property of its folder): a new one takes a name, whether
	its values are set by hand or come from a formula, and its type (a
	formula's, the type it returns; never a choice), with a choice's options
	(`OptionsEditor`, at least one); renaming one its name; changing its type
	the type, picked from tiles that wear each type's icon, with a warning that says what happens to
	the values (those that don't fit the new type are cleared; becoming a choice makes an option of
	each, and a choice's options go when it stops being one); editing a choice's options the options
	alone, each change saved as it is made, and deleting one asks first, saying how many documents
	will lose their value; and its formula the source and update mode (when changed, or
	continuously, every hour as well), which turns a property set by hand
	into a formula after a warning that its values will be deleted. A formula
	starts from an example written with the table's own columns. Adding the
	first column to a plain folder says it makes the folder a table. Adding a
	column, writing a formula and editing options open as a drawer (`SidePanel`); renaming and
	retyping, each a short question, as a dialog. The daemon's refusal shows
	under its field.
-->
<script lang="ts" module>
	import type {
		DocumentFolderProperty,
		FormulaDefinition,
		FormulaUpdateMode,
		IntelligenceAgentColor,
		NewPropertyOption,
		PropertyDataType
	} from '$lib/protocol/types';

	/** What the dialog is doing. */
	export type PropertyEdit =
		| { mode: 'new' }
		| { mode: 'rename'; property: DocumentFolderProperty }
		| { mode: 'type'; property: DocumentFolderProperty }
		| { mode: 'formula'; property: DocumentFolderProperty }
		| { mode: 'options'; property: DocumentFolderProperty };

	export interface PropertyFields {
		name: string;
		data_type: PropertyDataType;
		/** The formula, or null for values set by hand. */
		formula: FormulaDefinition | null;
		/** A new choice's options, in order; none for any other type. */
		options: NewPropertyOption[];
	}

	/** How the options of an existing choice change, each as it is made; each throws the daemon's refusal. */
	export interface OptionChanges {
		add: (property_id: number, name: string) => Promise<unknown>;
		rename: (property_id: number, option_id: number, name: string) => Promise<unknown>;
		color: (
			property_id: number,
			option_id: number,
			color: IntelligenceAgentColor | null
		) => Promise<unknown>;
		/** Moves an option just before `before_id`, or last when it's null. */
		move: (property_id: number, option_id: number, before_id: number | null) => Promise<unknown>;
		remove: (property_id: number, option_id: number) => Promise<unknown>;
		/** How many documents hold the option, or null when that isn't known (the rows shown are filtered). */
		usage: (property_id: number, option_id: number) => number | null;
	}
</script>

<script lang="ts">
	import { untrack, type Component } from 'svelte';
	import { ProtocolError, toProtocolError } from '$lib/protocol/errors';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import SidePanel from '$lib/ui/SidePanel.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import Clock from '@lucide/svelte/icons/clock';
	import PenLine from '@lucide/svelte/icons/pen-line';
	import SquareFunction from '@lucide/svelte/icons/square-function';
	import Zap from '@lucide/svelte/icons/zap';
	import { TYPE_ICONS } from './DocumentTable.svelte';
	import FormulaField from './FormulaField.svelte';
	import OptionsEditor, { type EditedOption } from './OptionsEditor.svelte';
	import TilePicker from './TilePicker.svelte';
	import { formulaStarter } from './formulas';
	import { optionsOf } from './values';

	interface Props {
		open?: boolean;
		edit: PropertyEdit | null;
		/** The folder's properties, which a formula's example reads; none when it isn't a table yet. */
		properties: DocumentFolderProperty[];
		onsubmit: (fields: PropertyFields) => Promise<unknown>;
		/** Changes an existing choice's options. */
		optionChanges: OptionChanges;
	}

	let { open = $bindable(false), edit, properties, onsubmit, optionChanges }: Props = $props();

	const SOURCES = [
		{ value: 'hand', label: 'Set by hand', icon: PenLine },
		{ value: 'formula', label: 'Formula', icon: SquareFunction }
	];
	const UPDATE_MODE_ICONS: Record<FormulaUpdateMode, Component> = {
		whenChanged: Zap,
		continuously: Clock
	};
	const updateModes = $derived(
		options(vocabulary.value.formula_update_modes).map((option) => ({
			...option,
			icon: UPDATE_MODE_ICONS[option.value]
		}))
	);
	const allTypes = $derived(
		options(vocabulary.value.property_data_types).map((option) => ({
			...option,
			icon: TYPE_ICONS[option.value as PropertyDataType]
		}))
	);

	let name = $state('');
	let dataType = $state<string>('text');
	/** Whether a new property's values come from a formula. */
	let values = $state('hand');
	let source = $state('');
	let updateMode = $state<string>('whenChanged');
	/** The type the example in `source` was written for. */
	let starterType = 'text';
	let error = $state<string | null>(null);
	/** Which field the refusal is of, so it shows under that one. */
	let errorField = $state<'name' | 'type' | 'formula' | 'options'>('type');
	let busy = $state(false);
	/** A new choice's options, until the column is added; ids below zero. */
	let newOptions = $state<EditedOption[]>([]);
	/** What is typed in the "Add option" line, which adding the column takes too. */
	let optionDraft = $state('');
	let nextOptionId = -1;

	const except = $derived(edit && edit.mode !== 'new' ? edit.property.id : null);
	const starter = (type: string) =>
		formulaStarter(
			type as PropertyDataType,
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
			updateMode = property?.formula?.update_mode ?? 'whenChanged';
			starterType = dataType;
			error = null;
			newOptions = [];
			optionDraft = '';
		});
	});

	const mode = $derived(edit?.mode ?? 'new');
	/** The property the formula is of, when the dialog edits one. */
	const formulaOf = $derived(edit?.mode === 'formula' ? edit.property : null);
	const converting = $derived(formulaOf !== null && formulaOf.formula === null);
	const withFormula = $derived(mode === 'formula' || (mode === 'new' && values === 'formula'));
	/** A formula is never a choice, so its type tiles leave it out. */
	const types = $derived(
		withFormula || (edit?.mode === 'type' && edit.property.formula !== null)
			? allTypes.filter((t) => t.value !== 'choice')
			: allTypes
	);
	/** The choice whose options the dialog edits, as it is now. */
	const choice = $derived.by(() => {
		if (edit?.mode !== 'options') return null;
		const id = edit.property.id;
		return properties.find((p) => p.id === id) ?? edit.property;
	});
	const choiceOptions = $derived(choice ? optionsOf(choice) : []);

	/**
	 * Writing a column or a formula takes the drawer, which has the room; a new
	 * name or type is a short question, so it stays a dialog.
	 */
	const drawer = $derived(mode === 'new' || mode === 'formula' || mode === 'options');

	/**
	 * Expanded, a formula's drawer is two columns: what the column is, then the
	 * formula with the height to write in. One set by hand has nothing to put
	 * beside it, so it stays one column, at the width of prose.
	 */
	const split = $derived(drawer && withFormula);

	/** The drawer's first control is Expand: start in the form instead. */
	function focusFirst(event: Event) {
		const first = document.querySelector<HTMLElement>(
			'#property-form input, #property-form textarea, #property-form [contenteditable="true"]'
		);
		if (!first) return;
		event.preventDefault();
		first.focus();
	}

	const title = $derived.by(() => {
		switch (edit?.mode) {
			case 'rename':
				return 'Rename column';
			case 'type':
				return `Change the type of “${edit.property.name}”?`;
			case 'formula':
				return converting
					? `Change “${edit.property.name}” to a formula?`
					: `Edit formula for “${edit.property.name}”`;
			case 'options':
				return `Edit options for “${edit.property.name}”`;
			default:
				return 'Add column';
		}
	});
	const unchanged = $derived(
		edit?.mode === 'rename'
			? name.trim() === edit.property.name
			: edit?.mode === 'type'
				? dataType === edit.property.data_type
				: edit?.mode === 'formula' &&
					source === edit.property.formula?.source &&
					updateMode === edit.property.formula.update_mode
	);
	const description = $derived.by(() => {
		if (edit?.mode === 'type' && dataType !== edit.property.data_type) {
			// A choice's values are its options' names, which text keeps.
			if (edit.property.data_type === 'choice')
				return dataType === 'text'
					? 'Each value will keep its option’s name. The options will be deleted.'
					: 'The options will be deleted. Values that don’t fit the new type will be cleared. This can’t be undone.';
			if (dataType === 'choice') return 'Each different value will become an option.';
			// Text reads every value, so only another type clears any.
			if (dataType !== 'text')
				return 'Values that don’t fit the new type will be cleared. This can’t be undone.';
		}
		if (converting)
			return `The values set by hand in “${formulaOf?.name}” will be deleted and replaced by what the formula returns. This can’t be undone.`;
		if (mode === 'new' && properties.length === 0)
			return 'This will convert this folder to a table.';
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
						: 'Add column'
	);

	/** A new formula's example follows its type until it is edited. */
	function retype(next: string) {
		if (source === starter(starterType)) source = starter(next);
		starterType = next;
		error = null;
	}

	/** The options a new choice would be added with: those listed, then what is typed to add. */
	const pendingOptions = $derived.by(() => {
		const typed = optionDraft.trim();
		const listed = newOptions.map(({ name, color }) => ({ name, color }));
		return typed === '' || listed.some((o) => sameName(o.name, typed))
			? listed
			: [...listed, { name: typed, color: null }];
	});

	const blank = $derived(
		(mode === 'new' || mode === 'rename') && name.trim() === ''
			? true
			: withFormula
				? source.trim() === ''
				: mode === 'new' && dataType === 'choice' && pendingOptions.length === 0
	);

	const sameName = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

	/** Refuses a name another of `list` has, other than `except`, as the daemon would. */
	function ensureUnique(list: EditedOption[], name: string, except: number | null = null) {
		if (list.some((o) => o.id !== except && sameName(o.name, name)))
			throw new ProtocolError(
				'validationFailed',
				`This column already has an option named “${name}”.`,
				'name'
			);
	}

	/** The changes the options editor makes: kept for a new column, saved for an existing one. */
	const optionHandlers = $derived.by(() => {
		if (choice) {
			const property_id = choice.id;
			return {
				onadd: (name: string) => optionChanges.add(property_id, name),
				onrename: (o: EditedOption, name: string) => optionChanges.rename(property_id, o.id, name),
				oncolor: (o: EditedOption, color: IntelligenceAgentColor | null) =>
					void optionChanges.color(property_id, o.id, color).catch((e) => toasts.error(e)),
				ondelete: (o: EditedOption) => (deletingOption = o),
				onreorder: (ids: number[]) => {
					const move = movedIn(
						choiceOptions.map((o) => o.id),
						ids
					);
					if (move)
						void optionChanges
							.move(property_id, move.id, move.before)
							.catch((e) => toasts.error(e));
				}
			};
		}
		return {
			onadd: async (name: string) => {
				ensureUnique(newOptions, name);
				newOptions.push({ id: nextOptionId--, name, color: null });
				error = null;
			},
			onrename: async (o: EditedOption, name: string) => {
				ensureUnique(newOptions, name, o.id);
				const at = newOptions.findIndex((x) => x.id === o.id);
				if (at >= 0) newOptions[at].name = name;
			},
			oncolor: (o: EditedOption, color: IntelligenceAgentColor | null) => {
				const at = newOptions.findIndex((x) => x.id === o.id);
				if (at >= 0) newOptions[at].color = color;
			},
			ondelete: (o: EditedOption) => (newOptions = newOptions.filter((x) => x.id !== o.id)),
			onreorder: (ids: number[]) =>
				(newOptions = ids.map((id) => newOptions.find((o) => o.id === id)!))
		};
	});

	/** The one item a drag moved from `before` to `after`, and the item it now goes before. */
	function movedIn(
		before: number[],
		after: number[]
	): { id: number; before: number | null } | null {
		for (const id of before) {
			const rest = (list: number[]) => list.filter((x) => x !== id).join();
			if (rest(before) === rest(after) && before.indexOf(id) !== after.indexOf(id)) {
				const at = after.indexOf(id);
				return { id, before: after[at + 1] ?? null };
			}
		}
		return null;
	}

	let deletingOption = $state<EditedOption | null>(null);
	/** What deleting an option does to the documents that hold it, as far as it is known. */
	const deletingText = $derived.by(() => {
		if (!deletingOption || !choice) return undefined;
		const count = optionChanges.usage(choice.id, deletingOption.id);
		if (count === null)
			return `Documents with this option will lose their value for “${choice.name}”.`;
		if (count === 0) return 'This can’t be undone.';
		if (count === 1) return `1 document will lose its value for “${choice.name}”.`;
		return `${count} documents will lose their value for “${choice.name}”.`;
	});

	async function submit(event?: SubmitEvent) {
		event?.preventDefault();
		if (blank || busy) return;
		busy = true;
		try {
			await onsubmit({
				name: name.trim(),
				data_type: dataType as PropertyDataType,
				formula: withFormula ? { source, update_mode: updateMode as FormulaUpdateMode } : null,
				options: mode === 'new' && dataType === 'choice' ? pendingOptions : []
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
							: failure.field === 'options'
								? 'options'
								: 'type';
			} else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet fields()}
	<form
		class={[
			'flex flex-1 flex-col gap-4',
			split
				? 'group-data-expanded/panel:min-h-0 group-data-expanded/panel:flex-row group-data-expanded/panel:gap-0'
				: 'group-data-expanded/panel:mx-auto group-data-expanded/panel:w-full group-data-expanded/panel:max-w-170'
		]}
		id="property-form"
		onsubmit={submit}
	>
		<!-- What the column is: beside the formula when the drawer is expanded, as a row's values sit beside its text. -->
		<div
			class={[
				'flex flex-col gap-4',
				split &&
					'group-data-expanded/panel:w-80 group-data-expanded/panel:flex-none group-data-expanded/panel:overflow-y-auto group-data-expanded/panel:border-r group-data-expanded/panel:border-border group-data-expanded/panel:p-5'
			]}
		>
			{#if drawer && description}<p class="text-fg-muted">{description}</p>{/if}
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
				<Field label="Values">
					<TilePicker
						label="Values"
						bind:value={values}
						tiles={SOURCES}
						columns={2}
						onchange={(next) => {
							error = null;
							// A formula is never a choice.
							if (next === 'formula' && dataType === 'choice') retype((dataType = 'text'));
						}}
					/>
				</Field>
			{/if}
			{#if mode === 'new' || mode === 'type'}
				<Field label="Type" error={errorField === 'type' ? error : null}>
					<TilePicker
						class={split ? 'group-data-expanded/panel:grid-cols-2' : undefined}
						label="Type"
						bind:value={dataType}
						tiles={types}
						columns={3}
						stacked
						onchange={retype}
					/>
				</Field>
			{/if}
			{#if (mode === 'new' && dataType === 'choice') || choice}
				<Field label="Options">
					<OptionsEditor
						options={choice ? choiceOptions : newOptions}
						bind:draft={optionDraft}
						error={errorField === 'options' ? error : null}
						{...optionHandlers}
					/>
				</Field>
			{/if}
			{#if withFormula}
				<Field label="Update mode">
					<TilePicker
						class={split ? 'group-data-expanded/panel:grid-cols-1' : undefined}
						label="Update mode"
						bind:value={updateMode}
						tiles={updateModes}
						columns={2}
						onchange={() => (error = null)}
					/>
				</Field>
			{/if}
		</div>
		{#if withFormula}
			<div
				class={[
					'flex flex-col',
					split &&
						'group-data-expanded/panel:min-w-0 group-data-expanded/panel:flex-1 group-data-expanded/panel:overflow-y-auto group-data-expanded/panel:p-5'
				]}
			>
				<FormulaField
					{source}
					type={dataType as PropertyDataType}
					error={errorField === 'formula' ? error : null}
					failure={source === formulaOf?.formula?.source ? formulaOf.formula.error : null}
					onchange={(text) => {
						source = text;
						error = null;
					}}
					onsubmit={() => void submit()}
				/>
			</div>
		{/if}
	</form>
{/snippet}

{#snippet settle()}
	{#if mode === 'options'}
		<!-- Each change is saved as it is made; the one button sits where Save would. -->
		<span></span>
		<Button variant="primary" onclick={() => (open = false)}>Done</Button>
	{:else}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="property-form"
			disabled={busy || unchanged || blank}
		>
			{submitLabel}
		</Button>
	{/if}
{/snippet}

{#if drawer}
	<SidePanel bind:open {title} {split} footer={settle} onOpenAutoFocus={focusFirst}>
		{@render fields()}
	</SidePanel>
{:else}
	<Dialog bind:open {title} {description} footer={settle}>
		{@render fields()}
	</Dialog>
{/if}

<ConfirmDialog
	subject={deletingOption}
	onclose={() => (deletingOption = null)}
	title="Delete “{deletingOption?.name ?? ''}”?"
	description={deletingText}
	onconfirm={(option) => optionChanges.remove(choice!.id, option.id)}
/>
