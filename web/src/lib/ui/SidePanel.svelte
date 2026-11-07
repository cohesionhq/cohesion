<!--
	A panel that slides in from the window's right edge over a dimmed page:
	a title row with `aside` after the title, `actions` and Close, then `children` in a column that
	scrolls on its own, then a `footer` toolbar when given. For reading or writing something longer than a dialog
	holds, without leaving the page. Escape and the backdrop close it.
-->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		open?: boolean;
		title: string;
		/** After the title, which wraps to leave it room. */
		aside?: Snippet;
		/** Beside Close, at the title row's end. */
		actions?: Snippet;
		/** A toolbar under the scrolling column, set off by a rule; its children spread to both ends. */
		footer?: Snippet;
		/** Fills the panel's flex column, which scrolls when it outgrows it. */
		children: Snippet;
		/** As bits-ui's: call `preventDefault` to focus something else than the first control. */
		onOpenAutoFocus?: (event: Event) => void;
	}

	let {
		open = $bindable(false),
		title,
		aside,
		actions,
		footer,
		children,
		onOpenAutoFocus
	}: Props = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop" />
		<Dialog.Content
			class="overlay-surface side-panel fixed top-[calc(var(--gutter)+var(--titlebar-inset))] right-(--gutter) bottom-(--gutter) z-50 flex w-[min(640px,calc(100vw-2*var(--gutter)))] flex-col outline-none"
			{onOpenAutoFocus}
		>
			<header class="flex flex-none items-start gap-2 border-b border-border py-3 pr-3 pl-5">
				<div class="flex min-w-0 flex-1 items-start gap-3">
					<Dialog.Title class="min-w-0 py-1 text-md font-semibold break-words">
						{title}
					</Dialog.Title>
					{#if aside}<div class="flex flex-none items-center">{@render aside()}</div>{/if}
				</div>
				{#if actions}<div class="flex flex-none items-center gap-1.5">{@render actions()}</div>{/if}
				<Dialog.Close
					class="inline-flex size-7.5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg"
					aria-label="Close"
				>
					<X size={16} strokeWidth={1.75} aria-hidden="true" />
				</Dialog.Close>
			</header>
			<div class="flex min-h-0 flex-1 flex-col overflow-y-auto p-5">{@render children()}</div>
			{#if footer}
				<footer
					class="flex flex-none items-center justify-between gap-1.5 border-t border-border px-3 py-2.5"
				>
					{@render footer()}
				</footer>
			{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
