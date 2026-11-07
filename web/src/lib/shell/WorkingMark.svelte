<!--
	A short accent hairline at the top centre of the window (in the title
	bar, where a WebUI harness has one), with a light running along it, while a
	request is in flight (see `working`). It waits before showing, so a
	near-instant one never flashes it: 300ms for an action the user set
	off, longer for a load, so a load only shows on a slow connection.
-->
<script lang="ts">
	import { fade } from 'svelte/transition';
	import { working } from '$lib/protocol/working.svelte';
	import { motion } from '$lib/ui/motion';

	const ACTION_DELAY = 300;
	const LOAD_DELAY = 500;

	let actionShown = $state(false);
	let loadShown = $state(false);

	/** Sets `show(true)` once `busy()` has held for `delay`, and `show(false)` once it ends. */
	function after(busy: () => boolean, delay: number, show: (shown: boolean) => void) {
		$effect(() => {
			if (!busy()) return;
			const timer = setTimeout(() => show(true), delay);
			return () => {
				clearTimeout(timer);
				show(false);
			};
		});
	}

	after(
		() => working.acting,
		ACTION_DELAY,
		(shown) => (actionShown = shown)
	);
	after(
		() => working.loading,
		LOAD_DELAY,
		(shown) => (loadShown = shown)
	);
</script>

<div
	class="pointer-events-none fixed top-[max(0.5rem,calc(var(--titlebar-inset)/2-1.5px))] left-1/2 z-60 -translate-x-1/2"
	role="status"
>
	{#if actionShown || loadShown}
		<span
			class="shimmer relative block h-0.75 w-4.5 overflow-hidden rounded-full bg-[color-mix(in_srgb,var(--accent)_25%,transparent)]"
			transition:fade={{ duration: motion.duration }}
		></span>
		<span class="sr-only">Working…</span>
	{/if}
</div>

<style>
	.shimmer::after {
		content: '';
		position: absolute;
		inset: 0;
		width: 8px;
		border-radius: inherit;
		background: var(--accent);
		animation: sweep 1.2s ease-in-out infinite;
	}

	@keyframes sweep {
		from {
			transform: translateX(-8px);
		}
		to {
			transform: translateX(18px);
		}
	}

	/* Holds still, mid-way, rather than parking out of sight. */
	@media (prefers-reduced-motion: reduce) {
		.shimmer::after {
			animation: none;
			transform: translateX(5px);
		}
	}
</style>
