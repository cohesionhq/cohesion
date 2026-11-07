<!--
	A card that floats beside its trigger while a mouse rests on it (or the
	trigger has keyboard focus), like a browser tab's preview. It adds
	detail only: everything in it must also be reachable without hovering.
	A row of them (the sidebar's agents) binds `open` to one shared value so
	only one shows, and passes `openDelay={0}` while one is open so moving
	along the row swaps cards at once.
-->
<script lang="ts">
	import { LinkPreview } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		open?: boolean;
		/** How long the pointer rests on the trigger before the card opens, in ms. */
		openDelay?: number;
		/** Renders the trigger, a link; spread `props` onto the `<a>`. */
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		side?: 'top' | 'right' | 'bottom' | 'left';
		align?: 'start' | 'center' | 'end';
		/** Keeps the card shut, e.g. while the trigger is dragged. */
		disabled?: boolean;
	}

	let {
		open = $bindable(false),
		openDelay = 500,
		trigger,
		children,
		side = 'right',
		align = 'start',
		disabled = false
	}: Props = $props();

	/** How long the pointer may rest off both the trigger and the card before it closes. */
	const STRAY_MS = 300;

	let triggerEl = $state<HTMLElement | null>(null);
	let contentEl = $state<HTMLElement | null>(null);

	// bits-ui keeps the card open while the pointer is anywhere in the
	// triangle between where it left the trigger and the card, so it can
	// cross to the card. A pointer that stops in that triangle over
	// something else (the next link down a column) would hold it open for
	// good; once it has rested off both for a moment, close.
	$effect(() => {
		if (!open) return;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const onMove = (event: PointerEvent) => {
			clearTimeout(timer);
			const target = event.target as Node;
			if (triggerEl?.contains(target) || contentEl?.contains(target)) return;
			timer = setTimeout(() => (open = false), STRAY_MS);
		};
		document.addEventListener('pointermove', onMove);
		return () => {
			clearTimeout(timer);
			document.removeEventListener('pointermove', onMove);
		};
	});
</script>

<LinkPreview.Root bind:open {openDelay} closeDelay={100} {disabled}>
	<LinkPreview.Trigger bind:ref={triggerEl}>
		{#snippet child({ props })}
			<!-- The trigger stays a link; bits-ui would make it a button. -->
			{@render trigger({ ...props, role: undefined })}
		{/snippet}
	</LinkPreview.Trigger>
	<LinkPreview.Portal>
		<LinkPreview.Content
			bind:ref={contentEl}
			class="overlay-surface z-45 w-64 max-w-[calc(100vw-2*var(--gutter))] p-3"
			{side}
			{align}
			sideOffset={8}
			collisionPadding={8}
		>
			{@render children()}
		</LinkPreview.Content>
	</LinkPreview.Portal>
</LinkPreview.Root>
