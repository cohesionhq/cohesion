<!--
	One row's value for one property, edited in place: a checkbox toggles, a
	choice shows as its option's pill and picks another, or none, from a menu
	of its options (each also on its number key, 0 clearing it) and a field that
	filters them as you type and, when the text names none, offers to create it
	and pick it, any
	other value set by hand opens an editor for its type on a click (Enter or a
	click elsewhere saves, Escape cancels, a value that can't be read or that
	the daemon refuses shows why under it), and a formula's value only reads.
	The table draws it as a cell (`look="cell"`); a row's panel as the value of
	one of its details (`look="detail"`), where an empty value says so. Its
	owner says which value is being edited (`editing`, `onedit`, `onstop`), so
	only one is at a time.
-->
<script lang="ts">
	import { tick } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { DocumentFolderProperty, PropertyValue } from '$lib/protocol/types';
	import { editable as detailButton } from '$lib/ui/ItemDetail.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import AgentDot from '$lib/features/agents/AgentDot.svelte';
	import ChoicePill from './ChoicePill.svelte';
	import { formatValue, inputText, isNumeric, optionOf, optionsOf, parseInput } from './values';

	interface Props {
		property: DocumentFolderProperty;
		value: PropertyValue | null;
		/** What names the row, for the control's accessible name: "Pages for Dune". */
		rowLabel: string;
		look: 'cell' | 'detail';
		/** Whether this value's editor is open. */
		editing: boolean;
		/** Asks to open this value's editor. */
		onedit: () => void;
		/** Asks to close it. */
		onstop: () => void;
		/** Sets the value, or empties it when null; throws the daemon's refusal. */
		onsave: (value: PropertyValue | null) => Promise<unknown>;
		/**
		 * Adds an option to a choice, last, and replies with the property; throws the refusal of its
		 * name. Without it the choice's menu has no search and add field.
		 */
		onaddoption?: (name: string) => Promise<DocumentFolderProperty>;
		/**
		 * Tab or Shift+Tab in the editor, once the value is saved: the table
		 * moves to the next value. Without it Tab moves focus as usual, which
		 * saves.
		 */
		ontab?: (back: boolean) => void;
	}

	let {
		property,
		value,
		rowLabel,
		look,
		editing,
		onedit,
		onstop,
		onsave,
		onaddoption,
		ontab
	}: Props = $props();

	const id = $props.id();
	const numeric = $derived(isNumeric(property.data_type));
	const options = $derived(optionsOf(property));
	const shown = $derived(value ? formatValue(value, options) : '');
	/** A choice's option, while it names one that exists. */
	const picked = $derived(value?.type === 'choice' ? optionOf(options, value.value) : null);
	const name = $derived(`${property.name} for ${rowLabel}`);

	let text = $state('');
	let error = $state<string | null>(null);
	let saving = $state(false);
	let input = $state<HTMLInputElement>();

	// The editor starts from the value each time it opens.
	let wasEditing = false;
	$effect(() => {
		if (editing && !wasEditing) void start();
		if (!editing) error = null;
		wasEditing = editing;
	});

	async function start() {
		text = inputText(value);
		error = null;
		await tick();
		input?.focus();
		if (property.data_type !== 'date') input?.select();
	}

	/**
	 * Saves the editor's value; true once it holds, false while it shows an
	 * error. It then closes the editor, unless `moving` on to another value.
	 */
	async function commit(moving = false): Promise<boolean> {
		const stop = () => {
			if (editing && !moving) onstop();
		};
		if (!editing || saving) return false;
		const parsed = parseInput(property.data_type, text);
		if ('error' in parsed) {
			error = parsed.error;
			return false;
		}
		if (JSON.stringify(parsed.value) === JSON.stringify(value)) {
			stop();
			return true;
		}
		saving = true;
		try {
			await onsave(parsed.value);
			// A click on another value may have moved the editor on while this saved.
			stop();
			return true;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed' && editing) {
				error = failure.message;
				return false;
			}
			toasts.error(failure);
			stop();
			return true;
		} finally {
			saving = false;
		}
	}

	async function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			onstop();
		} else if (event.key === 'Enter') {
			event.preventDefault();
			void commit();
		} else if (event.key === 'Tab' && ontab) {
			event.preventDefault();
			if (await commit(true)) ontab(event.shiftKey);
		}
	}

	function onblur() {
		// Escape and Tab have already settled the value; a click elsewhere saves it.
		if (editing) void commit();
	}

	function toggle(checked: boolean) {
		onsave({ type: 'checkbox', value: checked }).catch((e) => toasts.error(e));
	}

	/** Sets a choice to option `option_id`, or empties it when null. */
	function choose(option_id: number | null) {
		if (option_id === (picked?.id ?? null)) return;
		onsave(option_id === null ? null : { type: 'choice', value: option_id }).catch((e) =>
			toasts.error(e)
		);
	}

	/** What is typed in the choice menu's search field, and why the daemon refused it. */
	let optionName = $state('');
	let optionError = $state<string | null>(null);

	/** Adds the typed option, picks it for this value and closes the menu. */
	async function addOption(close: () => void, asToast = false) {
		const name = optionName.trim();
		if (name === '' || !onaddoption) return;
		try {
			const added = await onaddoption(name);
			const option = optionsOf(added).at(-1);
			optionName = '';
			close();
			if (option) choose(option.id);
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed' && !asToast) optionError = failure.message;
			else toasts.error(failure);
		}
	}

	/** The typed text, trimmed: the choice menu lists only the options that contain it. */
	const query = $derived(optionName.trim());
	const matches = $derived(
		query === ''
			? options
			: options.filter((o) => o.name.toLowerCase().includes(query.toLowerCase()))
	);
	/** Whether the typed text is yet to be an option, so the menu offers to create it. */
	const creatable = $derived(
		onaddoption !== undefined &&
			query !== '' &&
			!options.some((o) => o.name.toLowerCase() === query.toLowerCase())
	);

	/** A choice's menu: the options that match what is typed, checked when picked, then a way to empty it or create one. */
	const choiceItems = $derived<MenuItem[]>([
		...matches.map((option) => ({
			id: option.id,
			label: option.name,
			checked: option.id === picked?.id,
			key: query === '' ? optionKey(option.id) : undefined,
			onSelect: () => choose(option.id)
		})),
		...(options.length === 0 && query === ''
			? [{ id: 'none', label: 'No options yet', disabled: true, onSelect: () => {} }]
			: []),
		...(query === ''
			? [
					{
						id: 'clear',
						label: 'Clear',
						key: '0' as const,
						disabled: picked === null,
						separated: options.length > 0,
						onSelect: () => choose(null)
					}
				]
			: []),
		...(creatable
			? [
					{
						id: 'create',
						label: `Create ${property.name.toLowerCase()} "${query}"`,
						separated: matches.length > 0,
						onSelect: () => void addOption(() => {}, true)
					}
				]
			: [])
	]);

	/** An option's number key: its place in the menu, for the first nine. */
	function optionKey(id: number): string | undefined {
		const i = options.findIndex((o) => o.id === id);
		return i >= 0 && i < 9 ? String(i + 1) : undefined;
	}

	/** Focuses the search field as the menu opens, and clears it as the menu closes. */
	function searchField(node: HTMLInputElement) {
		node.focus();
		return {
			destroy() {
				optionName = '';
				optionError = null;
			}
		};
	}

	const cellPad = 'px-2.5 py-1.5';
</script>

{#snippet optionDot(item: MenuItem)}
	{@const option = options.find((o) => o.id === item.id)}
	{#if option?.color}
		<AgentDot color={option.color} />
	{:else if option}
		<span class="inline-block size-2 flex-none rounded-full border border-border-strong"></span>
	{:else}
		<span class="inline-block size-2 flex-none"></span>
	{/if}
{/snippet}

{#snippet addOptionField(close: () => void)}
	<div class="px-1">
		<input
			class="w-full rounded-sm border border-transparent bg-transparent px-1 py-1 outline-none placeholder:text-fg-muted hover:border-border focus:border-border-strong"
			placeholder="Search or add option…"
			aria-label="Search or add option"
			use:searchField
			aria-invalid={optionError !== null}
			autocomplete="off"
			bind:value={optionName}
			oninput={() => (optionError = null)}
			onkeydown={(event) => {
				// The menu would take typed keys for its own: a number key picks, a letter jumps.
				if (event.key !== 'Escape') event.stopPropagation();
				if (event.key === 'Enter') {
					event.preventDefault();
					void addOption(close);
				}
			}}
		/>
		{#if optionError}<p class="mt-1 px-1 text-xs text-danger" role="alert">{optionError}</p>{/if}
	</div>
{/snippet}

{#if property.data_type === 'choice' && property.formula === null}
	<!-- A detail sits in a row's panel, over the page, as a task's details do. -->
	<Menu
		items={choiceItems}
		align="start"
		icon={optionDot}
		inDialog={look === 'detail'}
		top={onaddoption ? addOptionField : undefined}
	>
		{#snippet trigger(props)}
			<button
				class={[
					look === 'cell'
						? ['flex min-h-8 w-full cursor-pointer items-center hover:bg-bg-hover', cellPad]
						: [detailButton, 'cursor-pointer', !picked && 'text-fg-muted'],
					'data-[state=open]:bg-bg-hover'
				]}
				type="button"
				aria-label={look === 'cell'
					? `${name}: ${picked?.name ?? 'empty'}`
					: `${property.name}: ${picked?.name ?? 'Empty'}`}
				{...props}
			>
				{#if picked}
					<ChoicePill option={picked} />
				{:else if look === 'detail'}
					Empty
				{/if}
			</button>
		{/snippet}
	</Menu>
{:else if property.data_type === 'checkbox'}
	<label class={['flex items-center', look === 'cell' ? cellPad : 'min-h-7']}>
		<input
			class="size-3.5 accent-accent disabled:cursor-default"
			type="checkbox"
			checked={value?.type === 'checkbox' && value.value}
			disabled={property.formula !== null}
			aria-label={name}
			onchange={(e) => toggle(e.currentTarget.checked)}
		/>
	</label>
{:else if editing}
	<input
		bind:this={input}
		bind:value={text}
		class={[
			'block min-w-0 outline-none',
			look === 'cell'
				? ['w-full bg-transparent', cellPad]
				: '-mx-2 h-7 w-[calc(100%+1rem)] rounded-sm bg-bg px-2 shadow-focus',
			numeric && 'tabular-nums',
			numeric && look === 'cell' && 'text-right'
		]}
		type={property.data_type === 'date' ? 'date' : 'text'}
		inputmode={property.data_type === 'integer' ? 'numeric' : numeric ? 'decimal' : undefined}
		autocomplete="off"
		aria-label={name}
		aria-invalid={error !== null}
		aria-describedby={error ? `${id}-error` : undefined}
		{onkeydown}
		{onblur}
		oninput={() => (error = null)}
	/>
	{#if error}
		<p
			class={[
				'text-left text-xs whitespace-normal text-danger',
				look === 'cell' ? 'max-w-60 px-2.5 pb-1.5' : 'w-full pt-1'
			]}
			id="{id}-error"
			role="alert"
		>
			{error}
		</p>
	{/if}
{:else if property.formula}
	{#if look === 'cell'}
		<span class={['block min-h-8 truncate text-fg-muted', cellPad]}>{shown}</span>
	{:else}
		<span class={['flex min-h-7 items-center text-fg-muted', numeric && 'tabular-nums']}
			>{shown || 'Empty'}</span
		>
	{/if}
{:else if look === 'cell'}
	<button
		class={[
			'block min-h-8 w-full cursor-text truncate hover:bg-bg-hover',
			cellPad,
			numeric ? 'text-right' : 'text-left'
		]}
		type="button"
		aria-label="{name}: {shown || 'empty'}"
		onclick={onedit}>{shown}</button
	>
{:else}
	<button
		class={[detailButton, 'cursor-text', numeric && 'tabular-nums', !value && 'text-fg-muted']}
		type="button"
		aria-label="{property.name}: {shown || 'Empty'}"
		onclick={onedit}>{shown || 'Empty'}</button
	>
{/if}
