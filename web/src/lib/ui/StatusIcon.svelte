<!--
	Where an actor session stands, as an icon, each a filled disc of one size:
	accent with a turning ring while working, an amber disc with an exclamation
	mark while it needs the user, and how it ended. A session that is done
	reads as ready (a filled green disc with a check) until its result has been
	seen, then as a grey one, and once resolved as a quiet double check; one
	that failed is a red disc with a cross. Every
	place that shows a session's state draws it with this, so the colors never
	drift apart.
-->
<script lang="ts">
	import CheckCheck from '@lucide/svelte/icons/check-check';
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
	<!-- A filled accent disc with three quarters of a ring cut out of it, turning, so working is a disc like the rest. -->
	<svg
		class="flex-none text-accent motion-safe:animate-spin"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="currentColor" />
		<path
			d="M12 6.5a5.5 5.5 0 1 1-5.5 5.5"
			fill="none"
			stroke="var(--bg)"
			stroke-width="2.5"
			stroke-linecap="round"
		/>
	</svg>
{:else if status === 'needsAttention'}
	<!-- A filled amber disc with an exclamation mark, asking for the user. -->
	<svg
		class="flex-none text-warn"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="currentColor" />
		<rect x="10.75" y="6.5" width="2.5" height="7.5" rx="1.25" fill="var(--bg)" />
		<circle cx="12" cy="17" r="1.5" fill="var(--bg)" />
	</svg>
{:else if status === 'idle' && unread}
	{@render checkDisc('text-ok')}
{:else if status === 'idle'}
	{@render checkDisc('text-n5')}
{:else if status === 'resolved'}
	<CheckCheck class="flex-none text-fg-subtle" {size} strokeWidth={1.75} aria-hidden="true" />
{:else}
	<!-- A filled red disc with a cross cut out of it. -->
	<svg
		class="flex-none text-danger"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="currentColor" />
		<path
			d="m8.75 8.75 6.5 6.5m0-6.5-6.5 6.5"
			fill="none"
			stroke="var(--bg)"
			stroke-width="2.5"
			stroke-linecap="round"
		/>
	</svg>
{/if}
