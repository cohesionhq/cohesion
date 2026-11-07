<!--
	Picks one value from a short list: a field-shaped trigger naming the
	choice, opening a list drawn like the app's menus, with a check on the
	current one. An entry with `options` of its own is a labelled group of
	them.
-->
<script lang="ts" module>
	export interface SelectOption {
		value: string;
		label: string;
	}
	export interface SelectGroup {
		label: string;
		options: SelectOption[];
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { Select } from 'bits-ui';
	import type { ClassValue } from 'svelte/elements';

	interface Props {
		value?: string;
		options: (SelectOption | SelectGroup)[];
		/** Called with the new value once the user picks one. */
		onchange?: (value: string) => void;
		disabled?: boolean;
		/** For a `Field`'s label. */
		id?: string;
		'aria-label'?: string;
		/** On the trigger, e.g. to fill a fixed-width column. */
		class?: ClassValue;
	}

	let {
		value = $bindable(''),
		options,
		onchange,
		disabled = false,
		id,
		'aria-label': ariaLabel,
		class: className
	}: Props = $props();

	const flat = $derived(options.flatMap((entry) => ('options' in entry ? entry.options : [entry])));
	const label = $derived(flat.find((option) => option.value === value)?.label ?? '');

	// Bits UI reads an empty value as nothing picked, but '' is a choice here
	// ("Not set"), so every value is keyed with a prefix inside the control.
	const key = (v: string) => `v:${v}`;
	const items = $derived(flat.map((option) => ({ value: key(option.value), label: option.label })));

	function pick(next: string) {
		value = next.slice(2);
		onchange?.(value);
	}

	const item =
		'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 outline-none data-highlighted:bg-bg-hover';
</script>

{#snippet option(entry: SelectOption)}
	<Select.Item class={item} value={key(entry.value)} label={entry.label}>
		{#snippet children({ selected })}
			<span class="min-w-0 flex-1 truncate">{entry.label}</span>
			<span class="flex w-3.5 flex-none">
				{#if selected}<Check size={14} strokeWidth={2} aria-hidden="true" />{/if}
			</span>
		{/snippet}
	</Select.Item>
{/snippet}

<Select.Root type="single" value={key(value)} {items} {disabled} onValueChange={pick}>
	<Select.Trigger
		{id}
		aria-label={ariaLabel}
		class={[
			'inline-flex h-8 max-w-full min-w-40 cursor-pointer items-center gap-2 rounded-sm border border-border-strong bg-bg px-2.5 text-left text-fg transition-colors hover:border-fg-subtle focus-visible:border-accent disabled:cursor-default disabled:opacity-50 data-[state=open]:border-accent',
			className
		]}
	>
		<span class="min-w-0 flex-1 truncate">{label}</span>
		<ChevronDown class="flex-none text-fg-subtle" size={14} strokeWidth={1.75} aria-hidden="true" />
	</Select.Trigger>
	<Select.Portal>
		<!-- Above dialogs, which some selects sit in. -->
		<Select.Content
			class="overlay-surface flyout z-55 max-h-(--bits-select-content-available-height) min-w-(--bits-select-anchor-width) overflow-y-auto p-1"
			sideOffset={4}
		>
			<Select.Viewport>
				{#each options as entry, i ('options' in entry ? `group-${i}` : entry.value)}
					{#if 'options' in entry}
						<Select.Group>
							<Select.GroupHeading class="px-2 pt-1.5 pb-0.5 text-xs text-fg-subtle">
								{entry.label}
							</Select.GroupHeading>
							{#each entry.options as child (child.value)}
								{@render option(child)}
							{/each}
						</Select.Group>
					{:else}
						{@render option(entry)}
					{/if}
				{/each}
			</Select.Viewport>
		</Select.Content>
	</Select.Portal>
</Select.Root>
