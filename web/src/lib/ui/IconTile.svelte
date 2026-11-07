<!--
	An extension's icon (shown for it, its services and their accounts): the
	image it gave, or else the title's initial on a tile whose color is picked
	from the title.
-->
<script lang="ts">
	import { tileColor } from '$lib/ui/colors';

	interface Props {
		icon: string | null;
		title: string;
		/** Width and height in pixels. */
		size?: number;
	}

	let { icon, title, size = 20 }: Props = $props();

	// Rounder as it grows, so a large tile reads as an app icon.
	const radius = $derived(`${Math.round(size / 4)}px`);
</script>

{#if icon}
	<img
		class="flex-none object-contain"
		style:border-radius={radius}
		src={icon}
		alt=""
		width={size}
		height={size}
		aria-hidden="true"
	/>
{:else}
	<span
		class={[
			'inline-flex flex-none items-center justify-center font-semibold text-on-fill',
			tileColor(title)
		]}
		style:width="{size}px"
		style:height="{size}px"
		style:border-radius={radius}
		style:font-size="{Math.round(size * 0.55)}px"
		aria-hidden="true"
	>
		{title.trim().charAt(0).toUpperCase() || '?'}
	</span>
{/if}
