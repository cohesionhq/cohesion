<!--
	Where an actor session stands, as an icon: a spinner while working, a pause
	in amber while it needs the user, and how it ended. A session that is done
	reads as ready (a filled accent disc with a check) until its result has been
	seen, then as a quiet check, and once resolved as a quiet double check. Every
	place that shows a session's state draws it with this, so the colors never
	drift apart.
-->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import CheckCheck from '@lucide/svelte/icons/check-check';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import type { ActorSessionStatus } from '$lib/protocol/types';

	interface Props {
		status: ActorSessionStatus;
		/** A done task whose result hasn't been seen yet. */
		unread?: boolean;
		/** Width and height in pixels. */
		size?: number;
	}

	let { status, unread = false, size = 16 }: Props = $props();
</script>

<!-- A filled disc in the given color with a check cut out of it, drawn like the pause. -->
{#snippet checkDisc(tone: string)}
	<svg
		class={['flex-none', tone]}
		width={size}
		height={size}
		viewBox="0 0 24 24"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="currentColor" />
		<path
			d="m7.75 12.25 2.75 2.75 5.75-6"
			fill="none"
			stroke="var(--bg)"
			stroke-width="2.5"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
	</svg>
{/snippet}

{#if status === 'working'}
	<LoaderCircle
		class="flex-none text-accent motion-safe:animate-spin"
		{size}
		strokeWidth={1.75}
		aria-hidden="true"
	/>
{:else if status === 'needsAttention'}
	<!-- A filled amber disc with a pause in it, so what needs the user stands out from what doesn't. -->
	<svg
		class="flex-none text-warn"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="currentColor" />
		<rect x="8.75" y="7.5" width="2.25" height="9" rx="1" fill="var(--bg)" />
		<rect x="13" y="7.5" width="2.25" height="9" rx="1" fill="var(--bg)" />
	</svg>
{:else if status === 'idle' && unread}
	{@render checkDisc('text-accent')}
{:else if status === 'idle'}
	<Check class="flex-none text-fg-subtle" {size} strokeWidth={1.75} aria-hidden="true" />
{:else if status === 'resolved'}
	<CheckCheck class="flex-none text-fg-subtle" {size} strokeWidth={1.75} aria-hidden="true" />
{:else}
	<CircleX class="flex-none text-danger" {size} strokeWidth={1.75} aria-hidden="true" />
{/if}
