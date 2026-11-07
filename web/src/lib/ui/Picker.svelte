<!--
	A popover that picks from a list by typing: the filter field takes focus
	on open, arrows move, Enter or a click picks. A single pick closes it; with
	`multiple` it stays open and each pick toggles. `create` offers a name no
	item has as one more choice.
-->
<script lang="ts" module>
	import type { Avatar } from './AgentAvatar.svelte';
	import type { Person } from './Photo.svelte';

	export interface PickerItem {
		id: string | number;
		label: string;
		checked: boolean;
		/** An agent's avatar or a person's photo before the label (e.g. who to assign a task to). */
		avatar?: Avatar | Person;
		/** A bare key that picks the item while the filter is empty, shown at its trailing edge. */
		key?: Key;
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Plus from '@lucide/svelte/icons/plus';
	import { Command, Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { keyLabel, pressesKey, type Key } from '$lib/shell/keys';
	import AvatarDisc from './Avatar.svelte';
	import { returnFocusFromKeyboard } from './returnFocus';
	import { toasts } from './toasts.svelte';

	interface Props {
		open?: boolean;
		items: PickerItem[];
		onpick: (id: PickerItem['id']) => void;
		/** Keeps the picker open after a pick, for choosing several. */
		multiple?: boolean;
		/** The filter field's placeholder and label. */
		placeholder: string;
		/** Offers to create an item named after the filter when none has that name. */
		create?: { label: (name: string) => string; run: (name: string) => Promise<unknown> };
		/** Shown when there is nothing to pick. */
		empty: string;
		/** A subtitle under `empty`: where to go when the list has nothing yet. */
		emptyDetail?: string;
		/** Opened from inside a dialog, so the picker must sit above it. */
		inDialog?: boolean;
		/** Where focus goes when the picker closes, instead of its trigger. */
		returnTo?: HTMLElement;
		/** Renders the trigger; spread `props` onto the element. */
		trigger: Snippet<[Record<string, unknown>]>;
	}

	let {
		open = $bindable(false),
		items,
		onpick,
		multiple = false,
		placeholder,
		create,
		empty,
		emptyDetail,
		inDialog = false,
		returnTo,
		trigger
	}: Props = $props();

	let query = $state('');
	let creating = $state(false);

	const name = $derived(query.trim());
	const shown = $derived(items.filter((i) => i.label.toLowerCase().includes(name.toLowerCase())));
	const offerCreate = $derived(
		create !== undefined &&
			name !== '' &&
			!items.some((i) => i.label.toLowerCase() === name.toLowerCase())
	);

	function pick(id: PickerItem['id']) {
		onpick(id);
		if (multiple) query = '';
		else open = false;
	}

	/** With nothing typed yet, an item's key picks it; once the filter has text, keys are typing. */
	function onfilterkey(event: KeyboardEvent) {
		if (query !== '') return;
		const hit = items.find((i) => i.key !== undefined && pressesKey(event, i.key));
		if (!hit) return;
		event.preventDefault();
		pick(hit.id);
	}

	async function runCreate() {
		if (create === undefined || creating) return;
		creating = true;
		try {
			await create.run(name);
			query = '';
			if (!multiple) open = false;
		} catch (error) {
			toasts.error(error);
		} finally {
			creating = false;
		}
	}

	const itemClass =
		'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-fg data-selected:bg-bg-hover [&_svg]:flex-none';

	$effect(() => {
		if (open) query = '';
	});
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</Popover.Trigger>
	<Popover.Portal>
		<Popover.Content
			class={[
				'overlay-surface w-60 max-w-[calc(100vw-2*var(--gutter))] overflow-hidden',
				inDialog ? 'z-55' : 'z-45'
			]}
			align="start"
			sideOffset={4}
			data-picker
			{...returnFocusFromKeyboard(returnTo)}
		>
			<Command.Root shouldFilter={false} loop>
				<Command.Input
					class="h-9 w-full border-b border-border px-3 text-fg placeholder:text-fg-subtle focus:shadow-none focus:outline-none"
					{placeholder}
					aria-label={placeholder}
					bind:value={query}
					onkeydown={onfilterkey}
				/>
				<Command.List class="max-h-[min(280px,50vh)] overflow-y-auto p-1">
					<Command.Viewport>
						{#each shown as item (item.id)}
							<Command.Item class={itemClass} value="item-{item.id}" onSelect={() => pick(item.id)}>
								{#if item.avatar}<AvatarDisc who={item.avatar} />{/if}
								<span class="min-w-0 flex-1 truncate">{item.label}</span>
								{#if item.key && name === ''}<kbd>{keyLabel(item.key)}</kbd>{/if}
								<span class="flex w-3.5 flex-none">
									{#if item.checked}<Check size={14} strokeWidth={2} aria-hidden="true" />{/if}
								</span>
							</Command.Item>
						{/each}
						{#if offerCreate && create}
							<Command.Item
								class={[itemClass, 'text-fg-muted']}
								value="create"
								disabled={creating}
								onSelect={runCreate}
							>
								<Plus size={14} strokeWidth={1.75} aria-hidden="true" />
								<span class="min-w-0 truncate">{create.label(name)}</span>
							</Command.Item>
						{:else if shown.length === 0}
							{#if name === '' && emptyDetail}
								<div class="flex flex-col gap-1 px-2 py-3 text-center">
									<p class="text-fg-muted">{empty}</p>
									<p class="text-xs text-fg-subtle">{emptyDetail}</p>
								</div>
							{:else}
								<p class="px-2 py-1.5 text-fg-subtle">{name === '' ? empty : 'No matches.'}</p>
							{/if}
						{/if}
					</Command.Viewport>
				</Command.List>
			</Command.Root>
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>
