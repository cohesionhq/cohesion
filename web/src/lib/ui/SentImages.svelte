<!--
	The images sent with a message, each opening near full screen on a
	click; a grey square
	stands in for each until they are fetched. The user's sit above their
	bubble at the trailing edge, an agent's or an actor's below what it
	said at the leading edge.
-->
<script lang="ts">
	import type { ImageUpload } from '$lib/protocol/types';
	import ImageViewer from './ImageViewer.svelte';
	import { imageUrl } from './images';

	interface Props {
		/** How many the message carries. */
		count: number;
		/** The images, once fetched. */
		images: (ImageUpload & { id: number })[] | undefined;
		/** Over the user's bubble, or under what an agent or an actor said. */
		place?: 'over' | 'under';
	}

	let { count, images, place = 'over' }: Props = $props();

	let viewing = $state<{ src: string; alt: string } | null>(null);

	const alt = (index: number) =>
		place === 'over' ? `Attached image ${index + 1}` : `Image ${index + 1}`;
</script>

<div
	class={[
		'flex max-w-[85%] flex-wrap gap-1.5 max-sm:max-w-full',
		place === 'over' ? 'mb-1 justify-end' : 'mt-1.5'
	]}
>
	{#if images}
		{#each images as image, index (image.id)}
			<button
				class="cursor-zoom-in overflow-hidden rounded-md border border-border"
				type="button"
				aria-label="Enlarge {alt(index).toLowerCase()}"
				onclick={() => (viewing = { src: imageUrl(image), alt: alt(index) })}
			>
				<img class="block max-h-60 max-w-60 object-cover" src={imageUrl(image)} alt={alt(index)} />
			</button>
		{/each}
	{:else}
		{#each { length: count }, index (index)}
			<span class="block size-32 rounded-md bg-n2" aria-label="Loading image"></span>
		{/each}
	{/if}
</div>

<ImageViewer bind:image={viewing} />
