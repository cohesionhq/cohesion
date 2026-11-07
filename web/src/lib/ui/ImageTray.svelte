<!--
	The images a composer will send with its message, as thumbnails in a
	row, each with a button taking it off.
-->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import type { ImageUpload } from '$lib/protocol/types';
	import { imageUrl } from './images';

	let { images = $bindable() }: { images: ImageUpload[] } = $props();
</script>

<ul class="flex flex-wrap gap-2 px-1.5 pt-1.5 pb-2" aria-label="Images to attach">
	{#each images as image, index (image)}
		<li class="relative">
			<img
				class="block size-14 rounded-md border border-border object-cover"
				src={imageUrl(image)}
				alt="Attachment {index + 1}"
			/>
			<button
				class="absolute -top-1.5 -right-1.5 inline-flex size-5 cursor-pointer items-center justify-center rounded-full border border-border bg-bg text-fg-muted hover:text-fg"
				type="button"
				aria-label="Remove image {index + 1}"
				onclick={() => (images = images.filter((_, i) => i !== index))}
			>
				<X size={12} strokeWidth={2} />
			</button>
		</li>
	{/each}
</ul>
