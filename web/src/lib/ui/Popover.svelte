<script lang="ts">
	import { Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { returnFocusFromKeyboard } from './returnFocus';

	interface Props {
		open?: boolean;
		/** Renders the trigger; spread `props` onto the element. */
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		align?: 'start' | 'center' | 'end';
		side?: 'top' | 'right' | 'bottom' | 'left';
		/** Opened from inside a dialog, so the panel must sit above it. */
		inDialog?: boolean;
		/** Flies out of the trigger as it fades in, rather than only fading. */
		flyout?: boolean;
	}

	let {
		open = $bindable(false),
		trigger,
		children,
		align = 'start',
		side = 'bottom',
		inDialog = false,
		flyout = false
	}: Props = $props();
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
				// Never taller than the room beside the trigger: a long panel scrolls
				// itself rather than the page. Kept a gutter (`--gutter`, 16px) from the
				// window's edges, as `collisionPadding` below says too.
				'overlay-surface max-h-[calc(var(--bits-popover-content-available-height)-var(--gutter))] max-w-[calc(100vw-2*var(--gutter))] min-w-50 overflow-y-auto overscroll-contain p-1',
				inDialog ? 'z-55' : 'z-45',
				flyout && 'flyout'
			]}
			{align}
			{side}
			sideOffset={4}
			collisionPadding={16}
			{...returnFocusFromKeyboard()}
		>
			{@render children()}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>
