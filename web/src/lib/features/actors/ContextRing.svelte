<!--
	How full a session's context window is, as a small ring: muted, so it never
	reads as a spinner, in the warning color past 80%, the danger color past 95%.
-->
<script lang="ts">
	import { usageTone } from './plan';

	let { share }: { share: number } = $props();

	// The ring: a 20px circle, its stroke 2.5px.
	const RADIUS = 8;
	const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
</script>

<svg class="flex-none -rotate-90" width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
	<circle class="stroke-n4" cx="10" cy="10" r={RADIUS} fill="none" stroke-width="2.5" />
	<circle
		class={{ calm: 'stroke-fg-muted', warn: 'stroke-warn', danger: 'stroke-danger' }[
			usageTone(share)
		]}
		cx="10"
		cy="10"
		r={RADIUS}
		fill="none"
		stroke-width="2.5"
		stroke-linecap="round"
		stroke-dasharray={CIRCUMFERENCE}
		stroke-dashoffset={CIRCUMFERENCE * (1 - share)}
	/>
</svg>
