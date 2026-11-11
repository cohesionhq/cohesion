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
		/**
		 * Draw it at the weight of the outline glyphs beside it (the sidebar's rows)
		 * instead of in its own colours. The treatment is a filter, so it holds for
		 * any logo, and lives here so no call site repeats it.
		 */
		quiet?: boolean;
	}

	let { icon, title, size = 20, quiet = false }: Props = $props();

	// Rounder as it grows, so a large tile reads as an app icon.
	const radius = $derived(`${Math.round(size / 4)}px`);
</script>

{#if icon}
	<img
		class={['flex-none object-contain', quiet && 'quiet']}
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
			quiet && 'quiet',
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

<style>
	/* Grey, but keeping most of the logo's own contrast, so its dark and light parts stay apart. */
	.quiet {
		filter: grayscale(1) contrast(0.8) brightness(1.1);
	}
	/* Lifted in the dark, so a black logo keeps its edge on the near-black canvas. */
	@media (prefers-color-scheme: dark) {
		.quiet {
			filter: grayscale(1) contrast(0.75) brightness(1.35);
		}
	}
</style>
