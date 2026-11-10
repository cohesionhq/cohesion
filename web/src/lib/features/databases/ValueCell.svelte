<!--
	One row's value for one property, edited in place: a checkbox toggles, any
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
	import type { DatabaseProperty, DatabaseValue } from '$lib/protocol/types';
	import { editable as detailButton } from '$lib/ui/ItemDetail.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { formatValue, inputText, isNumeric, parseInput } from './values';

	interface Props {
		property: DatabaseProperty;
		value: DatabaseValue | null;
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
		onsave: (value: DatabaseValue | null) => Promise<unknown>;
		/**
		 * Tab or Shift+Tab in the editor, once the value is saved: the table
		 * moves to the next value. Without it Tab moves focus as usual, which
		 * saves.
		 */
		ontab?: (back: boolean) => void;
	}

	let { property, value, rowLabel, look, editing, onedit, onstop, onsave, ontab }: Props = $props();

	const id = $props.id();
	const numeric = $derived(isNumeric(property.data_type));
	const shown = $derived(value ? formatValue(value) : '');
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

	const cellPad = 'px-2.5 py-1.5';
</script>

{#if property.data_type === 'checkbox'}
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
