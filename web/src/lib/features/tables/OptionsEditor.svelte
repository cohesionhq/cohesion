<!--
	A choice column's options, as a list to edit in place, laid out as the
	tags' list is: each row a color (`ColorPicker`), a name that reads as text
	until hovered or focused (Enter or leaving the field saves it, and Enter
	goes on to the next option), and a delete button; rows reorder by their
	handles or with Alt+↑/↓. The "Add option" line that ends the list adds
	one on Enter and stays ready for the next. Its owner keeps the options:
	a column being added holds them until it is created, an existing one
	saves each change as it is made.
-->
<script lang="ts" module>
	import type { IntelligenceAgentColor } from '$lib/protocol/types';

	/** An option as the editor shows it; one not yet saved has an id below zero. */
	export interface EditedOption {
		id: number;
		name: string;
		color: IntelligenceAgentColor | null;
	}
</script>

<script lang="ts">
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash from '@lucide/svelte/icons/trash-2';
	import { tick } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { Reorderable } from '$lib/ui/reorder.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import ColorPicker from '$lib/ui/ColorPicker.svelte';

	interface Props {
		options: EditedOption[];
		/** Adds an option last; throws the refusal of its name. */
		onadd: (name: string) => Promise<unknown>;
		/** Renames an option; throws the refusal of its name. */
		onrename: (option: EditedOption, name: string) => Promise<unknown>;
		oncolor: (option: EditedOption, color: IntelligenceAgentColor | null) => void;
		ondelete: (option: EditedOption) => void;
		/** Puts the options in this order of ids. */
		onreorder: (ids: number[]) => void;
		/** The refusal to show under the list, such as the daemon's of a new column's options. */
		error?: string | null;
		/** What is typed in the "Add option" line, not yet added. */
		draft?: string;
	}

	let {
		options,
		onadd,
		onrename,
		oncolor,
		ondelete,
		onreorder,
		error = null,
		draft: name = $bindable('')
	}: Props = $props();

	const id = $props.id();

	let addError = $state<string | null>(null);
	let busy = $state(false);
	let addField = $state<HTMLInputElement>();

	/** Gives the "Add option" field focus, to type the next one. */
	export function focusAdd() {
		addField?.focus();
	}

	async function add(event?: SubmitEvent) {
		event?.preventDefault();
		const trimmed = name.trim();
		if (trimmed === '' || busy) return;
		busy = true;
		try {
			await onadd(trimmed);
			name = '';
			await tick();
			addField?.focus();
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') addError = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}

	// Each row's field, as typed; a row not in it shows the option's own name.
	let drafts = $state<Record<number, string>>({});
	let rowErrors = $state<Record<number, string>>({});

	/** Saves a row's name; true once it holds. */
	async function rename(option: EditedOption): Promise<boolean> {
		const next = drafts[option.id]?.trim();
		if (next === undefined) return true;
		if (next === '' || next === option.name) {
			delete drafts[option.id];
			return true;
		}
		try {
			await onrename(option, next);
			delete drafts[option.id];
			delete rowErrors[option.id];
			return true;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed') rowErrors[option.id] = failure.message;
			else toasts.error(failure);
			return false;
		}
	}

	const order = new Reorderable(
		() => options,
		(option) => option.id,
		(ids) => onreorder(ids)
	);

	function onHandleDrag(event: DragEvent, option: EditedOption) {
		order.start(event, option, option.name);
		// The whole row follows the pointer, not only the handle.
		const row = (event.currentTarget as HTMLElement).closest('li')!;
		const { left, top } = row.getBoundingClientRect();
		event.dataTransfer!.setDragImage(row, event.clientX - left, event.clientY - top);
	}

	// A row's leading icon: an option's grip or the add row's plus.
	const handle = 'inline-flex h-8 w-4 flex-none items-center justify-center text-fg-subtle';
	// A name reads as text until it's hovered or focused.
	const field = 'border-transparent bg-transparent hover:border-border';
</script>

<div class="flex flex-col">
	{#if options.length}
		<ul class="flex flex-col" aria-label="Options" {...order.list}>
			{#each order.items as option, index (option.id)}
				<li
					class={[
						'group/option flex items-start gap-1 border-b border-border py-1',
						order.dragged === option.id && 'opacity-50'
					]}
					{...order.target(index)}
				>
					<span
						class={[
							handle,
							'cursor-grab opacity-0 transition-opacity group-focus-within/option:opacity-100 group-hover/option:opacity-100 hover:text-fg touch:opacity-100'
						]}
						draggable="true"
						aria-hidden="true"
						ondragstart={(e) => onHandleDrag(e, option)}
						ondragend={() => order.end()}
					>
						<GripVertical size={14} strokeWidth={1.75} />
					</span>
					<ColorPicker
						color={option.color}
						label="Color of {option.name}"
						onchange={(color) => oncolor(option, color)}
					/>
					<div class="min-w-0 flex-1">
						<TextInput
							class={field}
							value={drafts[option.id] ?? option.name}
							aria-label="Name of {option.name}"
							aria-invalid={rowErrors[option.id] !== undefined}
							autocomplete="off"
							oninput={(event) => {
								drafts[option.id] = event.currentTarget.value;
								delete rowErrors[option.id];
							}}
							onblur={() => rename(option)}
							aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
							onkeydown={async (event) => {
								order.key(event, index);
								if (event.key === 'Enter') {
									event.preventDefault();
									if (await rename(option)) addField?.focus();
								}
							}}
						/>
						{#if rowErrors[option.id]}
							<p class="mx-2 mt-1 mb-1 text-xs text-danger">{rowErrors[option.id]}</p>
						{/if}
					</div>
					<button
						class="inline-flex size-8 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-subtle opacity-0 transition-opacity group-focus-within/option:opacity-100 group-hover/option:opacity-100 hover:bg-bg-hover hover:text-danger focus-visible:opacity-100 touch:opacity-100"
						type="button"
						aria-label="Delete {option.name}"
						onclick={() => ondelete(option)}
					>
						<Trash size={15} strokeWidth={1.75} />
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	<!-- Not a form: the editor sits inside the column's own form. -->
	<div class="flex items-start gap-1 py-1">
		<span class={handle} aria-hidden="true">
			<Plus size={14} strokeWidth={1.75} />
		</span>
		<!-- Lines the field up with the names above, past their colors. -->
		<span class="size-8 flex-none"></span>
		<div class="min-w-0 flex-1">
			<TextInput
				bind:element={addField}
				class={field}
				bind:value={name}
				placeholder="Add option"
				aria-label="Add option"
				aria-invalid={addError !== null}
				aria-describedby={addError || error ? `${id}-error` : undefined}
				autocomplete="off"
				oninput={() => (addError = null)}
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						event.preventDefault();
						void add();
					}
				}}
			/>
		</div>
		<!-- Lines the field up with the names above, past their delete buttons. -->
		<span class="size-8 flex-none"></span>
	</div>
	{#if addError || error}
		<p class="mx-2 mt-1 mb-1 pl-17 text-xs text-danger" id="{id}-error" role="alert">
			{addError ?? error}
		</p>
	{/if}
</div>
