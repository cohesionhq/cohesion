<!--
	A row of flat, underlined chips (MenuBarItem) that share one panel
	under them. With the panel open, clicking or pointing at another chip
	moves it there: it slides across and resizes to the new contents,
	which slide in from that side, rather than closing and opening again.
-->
<script lang="ts">
	import { Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { fly } from 'svelte/transition';
	import { MenuBarState, setMenuBar } from './menuBar.svelte';
	import { motion } from './motion';

	interface Props {
		/** The chips, each a MenuBarItem. */
		children: Snippet;
	}

	let { children }: Props = $props();

	const bar = setMenuBar(new MenuBarState());

	let row = $state<HTMLElement>();
	let width = $state(0);
	let height = $state(0);
	let viaKeyboard = false;
</script>

<div class="flex min-w-0 flex-wrap items-center gap-x-3" bind:this={row}>
	{@render children()}
</div>

<Popover.Root bind:open={bar.open}>
	<Popover.Portal>
		<Popover.Content
			class="menu-bar-panel overlay-surface z-45 max-h-[calc(var(--bits-popover-content-available-height)-var(--gutter))] max-w-[calc(100vw-2*var(--gutter))] overflow-x-hidden overflow-y-auto overscroll-contain p-1"
			customAnchor={bar.active?.anchor ?? null}
			side="bottom"
			align="start"
			sideOffset={6}
			data-moved={bar.moved || undefined}
			style="--slide: {motion.duration}ms"
			onInteractOutside={(event) => {
				// A chip moves the panel itself; this would close it first.
				if (row?.contains(event.target as Node)) event.preventDefault();
			}}
			onkeydown={() => (viaKeyboard = true)}
			onpointerdown={() => (viaKeyboard = false)}
			onCloseAutoFocus={(event) => {
				event.preventDefault();
				if (viaKeyboard) bar.active?.anchor.focus();
				viaKeyboard = false;
			}}
		>
			<!-- Sized to the contents in pixels, so a move can ease between two sizes. -->
			<div
				class="sizer overflow-hidden"
				style:width={width ? `${width}px` : undefined}
				style:height={height ? `${height}px` : undefined}
			>
				<div class="w-max" bind:offsetWidth={width} bind:offsetHeight={height}>
					{#key bar.active?.id}
						<div in:fly={{ x: bar.direction * 24, duration: motion.duration }}>
							{@render bar.active?.panel(bar.close)}
						</div>
					{/key}
				</div>
			</div>
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>

<style>
	/* Only once moved: the first placement comes from off-screen. */
	:global([data-bits-floating-content-wrapper]:has(> .menu-bar-panel[data-moved])) {
		transition: transform var(--slide) ease-out;
	}

	:global(.menu-bar-panel[data-moved]) .sizer {
		transition:
			width var(--slide) ease-out,
			height var(--slide) ease-out;
	}
</style>
