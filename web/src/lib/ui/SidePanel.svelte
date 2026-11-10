<!--
	A panel that slides in from the window's right edge over a dimmed page:
	a title row with `aside` after the title, `actions`, Expand and Close, then `children` in a column that
	scrolls on its own, then a `footer` toolbar when given. For reading or writing something longer than a dialog
	holds, without leaving the page. Escape and the backdrop close it.
	Expand floats the panel at the middle of the window, wider and clear of its edges, for what needs more
	room; the same button returns it. A window too narrow to have room to give has no button. The panel
	is expanded for one opening only: each opening starts as the drawer.
	Inside it, `group-data-expanded/panel:` styles what the wider panel changes. A `split` panel lays
	its children out in columns that scroll on their own when expanded, so the column leaves the
	scrolling and the padding to them.
-->
<script lang="ts">
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Minimize2 from '@lucide/svelte/icons/minimize-2';
	import X from '@lucide/svelte/icons/x';
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';

	interface Props {
		open?: boolean;
		title: string;
		/** After the title, which wraps to leave it room. */
		aside?: Snippet;
		/** Beside Close, at the title row's end. */
		actions?: Snippet;
		/** A toolbar under the scrolling column, set off by a rule; its children spread to both ends. */
		footer?: Snippet;
		/** The children are columns of their own when expanded: the column neither pads nor scrolls then. */
		split?: boolean;
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
		split = false,
		children,
		onOpenAutoFocus
	}: Props = $props();

	// From `max-md:`'s window width (tokens.css) up, where the drawer leaves room to grow into.
	const roomy = new MediaQuery('min-width: 900px');
	let expanded = $state(false);
	// Nothing is kept: every opening starts as the drawer.
	$effect.pre(() => {
		if (open) expanded = false;
	});
	const wide = $derived(expanded && roomy.current);

	const buttonClass =
		'inline-flex size-7.5 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg';
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop" />
		<Dialog.Content
			class="overlay-surface side-panel group/panel fixed z-50 flex flex-col outline-none"
			data-expanded={wide ? '' : undefined}
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
				{#if roomy.current}
					<button
						class={buttonClass}
						type="button"
						aria-label={expanded ? 'Minimize' : 'Expand'}
						onclick={() => (expanded = !expanded)}
					>
						{#if expanded}
							<Minimize2 size={16} strokeWidth={1.75} aria-hidden="true" />
						{:else}
							<Maximize2 size={16} strokeWidth={1.75} aria-hidden="true" />
						{/if}
					</button>
				{/if}
				<Dialog.Close class={buttonClass} aria-label="Close">
					<X size={16} strokeWidth={1.75} aria-hidden="true" />
				</Dialog.Close>
			</header>
			<div
				class={[
					'flex min-h-0 flex-1 flex-col overflow-y-auto p-5 transition-[padding] duration-(--duration)',
					split
						? 'group-data-expanded/panel:overflow-hidden group-data-expanded/panel:p-0'
						: 'group-data-expanded/panel:px-10 group-data-expanded/panel:py-8'
				]}
			>
				{@render children()}
			</div>
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
