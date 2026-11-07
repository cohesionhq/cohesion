<!--
	A photo of someone, a contact or the user, on a disc: 20px beside a
	label (a menu item, a task), 28px in a list row, where someone without
	one gets an empty disc so the names line up, and 96px on its own (a
	contact's panel, Settings › Profile). Without a photo, `name` puts its
	initials on the disc, at the agents' share for an initial (0.5).
-->
<script lang="ts" module>
	import type { Photo } from '$lib/protocol/types';

	/** Someone drawn by their photo, or by their name's initials when they have none. */
	export interface Person {
		name: string | null;
		photo: Photo;
	}
</script>

<script lang="ts">
	import User from '@lucide/svelte/icons/user';
	import { imageUrl } from '$lib/ui/images';
	import { initials } from './initials';

	interface Props {
		photo: Photo;
		/** Drawn as initials when there is no photo. */
		name?: string | null;
		/** `xs` beside a label, `sm` for list rows, `xl` on its own. */
		size?: 'xs' | 'sm' | 'xl';
	}

	let { photo, name = null, size = 'sm' }: Props = $props();

	const disc = { xs: 20, sm: 28, xl: 96 };
	/** An initial's font size as a share of the disc's diameter, as an agent's. */
	const INITIAL_SHARE = 0.5;
	const letters = $derived(initials(name));
</script>

<span
	class="inline-flex size-(--disc) flex-none items-center justify-center overflow-hidden rounded-full bg-bg-hover leading-none font-semibold whitespace-nowrap text-fg-muted select-none"
	style:--disc="{disc[size]}px"
	style:font-size="{disc[size] * INITIAL_SHARE}px"
	aria-hidden="true"
>
	{#if photo.kind === 'image'}
		<img
			class="size-full object-cover"
			src={imageUrl({ media_type: photo.photo_media_type, data: photo.photo_data })}
			alt=""
		/>
	{:else if letters}
		{letters}
	{:else}
		<User size={disc[size] / 2} strokeWidth={1.75} />
	{/if}
</span>
