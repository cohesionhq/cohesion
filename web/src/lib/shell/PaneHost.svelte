<!--
	One half of the sheet, scrolling on its own: makes `pane` the half the page
	inside reads and moves, and gives the page's header the split view's
	controls. A link clicked in the second half opens there; ⌥-clicking a link
	anywhere opens it in the second half (the root layout).
-->
<script lang="ts">
	import Columns2 from '@lucide/svelte/icons/columns-2';
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import { providePageStores, type PageStores } from '$lib/stores/pageStores.svelte';
	import { setPaneChrome } from '$lib/ui/paneChrome';
	import { panes, providePane, type Pane, type Side } from './pane.svelte';
	import { internalHref } from './links';

	interface Props {
		side: Side;
		pane: Pane;
		/** The stores its page uses; the main half's when not given. */
		stores?: PageStores;
		/** The element the half is drawn in, which a navigation fades. */
		root?: HTMLElement;
		children: Snippet;
	}

	let { side, pane, stores, root = $bindable(), children }: Props = $props();

	// Each half keeps the pane and stores it was drawn for.
	// svelte-ignore state_referenced_locally
	const here = pane;
	providePane(here);
	// svelte-ignore state_referenced_locally
	if (stores) providePageStores(stores);
	// svelte-ignore state_referenced_locally
	const hasGutter = side === 'main';
	setPaneChrome({
		get back() {
			return here.state.back;
		},
		goBack: () => here.step(-1),
		controls,
		hasGutter
	});

	function focus() {
		panes.focused = side;
	}

	// A plain click on a link in the second half opens it there, not in the main half.
	function onclick(event: MouseEvent) {
		if (side !== 'split' || event.defaultPrevented || event.button !== 0) return;
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		const href = internalHref(event.target);
		if (href === null) return;
		event.preventDefault();
		void here.goto(href);
	}

	const buttonClass =
		'inline-flex size-7 flex-none cursor-pointer items-center justify-center rounded-sm text-fg-muted transition-colors hover:bg-bg-hover hover:text-fg';
</script>

{#snippet controls()}
	{#if panes.split}
		<!-- Closing the main half while the second is blank would leave nothing; close the second instead. -->
		{#if side === 'split' || panes.split.href}
			<button class={buttonClass} aria-label="Close this half" onclick={() => panes.close(side)}>
				<X size={16} strokeWidth={1.75} />
			</button>
		{/if}
	{:else}
		<!-- A phone's screen is too narrow for two pages. -->
		<button
			class={[buttonClass, 'max-sm:hidden']}
			aria-label="Split view"
			onclick={() => panes.openSplit()}
		>
			<Columns2 size={16} strokeWidth={1.75} />
		</button>
	{/if}
{/snippet}

<!-- The second half's headers keep out of a navigation's fade by name: two headers can't share one.
     The click handler only catches clicks on the links inside, which are focusable themselves. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<svelte:element
	this={side === 'main' ? 'main' : 'section'}
	bind:this={root}
	class={[
		'relative min-h-0 min-w-0 flex-1 [scrollbar-width:thin] overflow-y-auto rounded-(--sheet-radius) pb-12',
		side === 'split' &&
			'rounded-l-none border-l border-border [&_header]:[view-transition-name:none]',
		// Split, each half lays out by its own width as well as the window's (`tokens.css`).
		panes.split && '@container/pane',
		side === 'main' && panes.split && 'rounded-r-none'
	]}
	aria-label={side === 'split' ? 'Split view' : undefined}
	data-view-page="/"
	onpointerdowncapture={focus}
	onfocusincapture={focus}
	{onclick}
>
	{@render children()}
</svelte:element>
