<!--
	One image, or one rendered diagram, near full screen over a dimmed page.
	An image is as large as fits the window, never larger than it is; a
	diagram scales to fill it. A click anywhere, Escape or the close button
	puts it away.
-->
<script module lang="ts">
	/** An image, or a diagram given as its Mermaid text and the width it is drawn at on the page. */
	export type ViewerItem =
		{ src: string; alt: string } | { diagram: string; shownAt: number; alt: string };
</script>

<script lang="ts">
	import { Dialog } from 'bits-ui';
	import X from '@lucide/svelte/icons/x';
	import type { Attachment } from 'svelte/attachments';
	import { mountDiagram } from './diagram';

	interface Props {
		/** The image or diagram shown, or `null` when none is. */
		image: ViewerItem | null;
	}

	let { image = $bindable() }: Props = $props();

	/** Kept while it fades out, so the image doesn't vanish before its overlay. */
	let shown = $state<ViewerItem | null>(null);
	$effect(() => {
		if (image) shown = image;
	});

	/** Draws the diagram in the whole canvas, by its width. */
	const drawn =
		(source: string, shownAt: number): Attachment<HTMLElement> =>
		(element) =>
			void mountDiagram(element, source, shownAt);
</script>

{#snippet close(position: string)}
	<Dialog.Close
		class="absolute {position} inline-flex size-9 cursor-pointer items-center justify-center rounded-full bg-(--bg-floating) text-fg hover:bg-(--bg-hover)"
		aria-label="Close"
	>
		<X size={18} strokeWidth={1.75} aria-hidden="true" />
	</Dialog.Close>
{/snippet}

<Dialog.Root
	open={image !== null}
	onOpenChange={(open) => open || (image = null)}
	onOpenChangeComplete={(open) => open || (shown = null)}
>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop backdrop-blur-sm" />
		<Dialog.Content
			class="fixed inset-0 z-50 flex cursor-zoom-out items-center justify-center p-[max(var(--gutter),2.5vmin)] pt-[calc(max(var(--gutter),2.5vmin)+var(--titlebar-inset))] outline-none data-[state=closed]:animate-[fade-out_var(--duration)_ease-in] data-[state=open]:animate-[fade-in_var(--duration)_ease-out]"
			onclick={() => (image = null)}
		>
			<Dialog.Title class="sr-only">
				{shown?.alt || ('diagram' in (shown ?? {}) ? 'Diagram' : 'Image')}
			</Dialog.Title>
			{#if shown && 'diagram' in shown}
				<!-- The diagram's own canvas: flat, with the close button inside its corner. -->
				<div
					class="diagram relative flex size-full items-center justify-center rounded-md bg-(--bg-raised) p-6"
					role="img"
					aria-label={shown.alt}
				>
					<div
						class="flex size-full items-center justify-center"
						{@attach drawn(shown.diagram, shown.shownAt)}
					></div>
					{@render close('top-3 right-3')}
				</div>
			{:else if shown}
				<img
					class="max-h-full max-w-full rounded-md object-contain shadow-[var(--shadow-floating)]"
					src={shown.src}
					alt={shown.alt}
				/>
				{@render close('top-[calc(var(--gutter)+var(--titlebar-inset))] right-(--gutter)')}
			{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
