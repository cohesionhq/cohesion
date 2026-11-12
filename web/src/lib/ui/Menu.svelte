<script lang="ts" module>
	import type { Avatar } from './AgentAvatar.svelte';
	import type { Person } from './Photo.svelte';

	interface MenuEntry {
		/** A stable key when labels can repeat (e.g. user-named rows); defaults to `label`. */
		id?: string | number;
		label: string;
		disabled?: boolean;
		/** Draws a rule above the item, starting a new group. */
		separated?: boolean;
		/** Quiet text under the label (e.g. why it is disabled). */
		detail?: string;
	}

	/** An item that acts when chosen. */
	export interface MenuAction extends MenuEntry {
		onSelect: () => void;
		/** The name assistive tech reads when the label leans on the submenu it sits in ("Size: Large"). */
		ariaLabel?: string;
		/**
		 * A bare key that picks the item while the menu is open, shown at its
		 * trailing edge: a letter or digit as typed, a named key (`'backspace'`).
		 */
		key?: Key;
		danger?: boolean;
		/** Marks a choice: true draws a check at the item's trailing edge, false keeps its space. */
		checked?: boolean;
		/** An agent's avatar or a person's photo before the label (e.g. the agents to forward to). */
		avatar?: Avatar | Person;
	}

	/** An item that opens a submenu of actions beside it (e.g. the folders to move to). */
	export interface MenuSubmenu extends MenuEntry {
		items: MenuAction[];
	}

	export type MenuItem = MenuAction | MenuSubmenu;
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import AvatarDisc from './Avatar.svelte';
	import { keyLabel, pressesKey, type Key } from '$lib/shell/keys';
	import { returnFocusFromKeyboard } from './returnFocus';

	interface Props {
		/** Renders the trigger; spread `props` onto the element. */
		trigger: Snippet<[Record<string, unknown>]>;
		items: MenuItem[];
		align?: 'start' | 'center' | 'end';
		/** Quiet text above the items (e.g. what the menu is about). */
		header?: Snippet;
		/** Drawn before each item's label (e.g. a service's icon). */
		icon?: Snippet<[MenuItem]>;
		/** Opened from inside a dialog, so the menu must sit above it. */
		inDialog?: boolean;
		/** Before the items, over a rule (e.g. a field to search them); `close` closes the menu. */
		top?: Snippet<[() => void]>;
	}

	let { trigger, items, align = 'end', header, icon, inDialog = false, top }: Props = $props();

	let open = $state(false);
	const returnFocus = returnFocusFromKeyboard();

	/** Picks the item whose key was pressed; the menu holds focus, so this never meets a field. */
	function onkeydown(event: KeyboardEvent) {
		returnFocus.onkeydown();
		const hit = items
			.flatMap((item) => ('items' in item ? [] : [item]))
			.find((item) => item.key !== undefined && !item.disabled && pressesKey(event, item.key));
		if (!hit) return;
		event.preventDefault();
		event.stopPropagation();
		open = false;
		hit.onSelect();
	}

	const surface = $derived(['overlay-surface min-w-40 p-1', inDialog ? 'z-55' : 'z-45']);

	const row =
		'cursor-pointer rounded-sm px-2 py-1 outline-none data-disabled:cursor-default data-disabled:opacity-50 data-highlighted:bg-bg-hover';
</script>

{#snippet text(item: MenuItem)}
	<span class="flex min-w-0 flex-col">
		<span>{item.label}</span>
		{#if item.detail}<span class="max-w-64 text-xs text-fg-muted">{item.detail}</span>{/if}
	</span>
{/snippet}

{#snippet actionItem(item: MenuAction)}
	{@const lead = icon || item.avatar}
	<DropdownMenu.Item
		class={[
			row,
			item.danger && 'text-danger',
			(lead || item.detail || item.checked !== undefined || item.key) && 'flex items-start gap-2'
		]}
		disabled={item.disabled}
		aria-label={item.ariaLabel}
		onSelect={item.onSelect}
	>
		{#if lead || item.detail || item.checked !== undefined || item.key}
			{#if item.avatar}
				<span class="flex h-5 flex-none items-center"><AvatarDisc who={item.avatar} /></span>
			{:else if icon}
				<span class="flex h-5 flex-none items-center">{@render icon(item)}</span>
			{/if}
			{@render text(item)}
			{#if item.key}
				<kbd class="ml-auto flex-none self-center">{keyLabel(item.key)}</kbd>
			{/if}
			{#if item.checked !== undefined}
				<span class={[item.key ? 'ml-1' : 'ml-auto', 'flex h-5 w-3.5 flex-none items-center']}>
					{#if item.checked}<Check size={14} strokeWidth={2} aria-hidden="true" />{/if}
				</span>
			{/if}
		{:else}
			{item.label}
		{/if}
	</DropdownMenu.Item>
{/snippet}

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content class={surface} {align} sideOffset={4} {...returnFocus} {onkeydown}>
			{#if header}
				<div class="max-w-72 px-2 py-1.5 text-xs text-fg-muted">{@render header()}</div>
				<DropdownMenu.Separator class="-mx-1 my-1 h-px bg-border" />
			{/if}
			{#if top}
				{@render top(() => (open = false))}
				<DropdownMenu.Separator class="-mx-1 my-1 h-px bg-border" />
			{/if}
			{#each items as item (item.id ?? item.label)}
				{#if item.separated}
					<DropdownMenu.Separator class="-mx-1 my-1 h-px bg-border" />
				{/if}
				{#if 'items' in item}
					<DropdownMenu.Sub>
						<DropdownMenu.SubTrigger
							class={[row, 'flex items-start gap-2 data-[state=open]:bg-bg-hover']}
							disabled={item.disabled}
						>
							{#if icon}<span class="flex h-5 flex-none items-center">{@render icon(item)}</span
								>{/if}
							{@render text(item)}
							<span class="ml-auto flex h-5 flex-none items-center text-fg-subtle">
								<ChevronRight size={14} strokeWidth={1.75} aria-hidden="true" />
							</span>
						</DropdownMenu.SubTrigger>
						<DropdownMenu.SubContent class={surface} sideOffset={4}>
							{#each item.items as action (action.id ?? action.label)}
								{@render actionItem(action)}
							{/each}
						</DropdownMenu.SubContent>
					</DropdownMenu.Sub>
				{:else}
					{@render actionItem(item)}
				{/if}
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
