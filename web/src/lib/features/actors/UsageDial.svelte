<!--
	A plan limit as a dial: a ring filled as far as a share of the limit is
	used, with the percentage inside, the limit's name beside it and, when
	known, when it resets under the name. The ring is in the accent, then the warning color
	past 80%, the danger color past 95%.
-->
<script lang="ts">
	import { usagePercent, usageTone } from './plan';

	let {
		share,
		label,
		resets = null
	}: { share: number; label: string; resets?: string | null } = $props();

	// The dial: a 36px circle, its stroke 3.5px.
	const RADIUS = 16;
	const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
	const filled = $derived(Math.min(Math.max(share, 0), 1));
</script>

<div class="flex flex-none items-center gap-2">
	<div class="relative size-9">
		<svg class="-rotate-90" width="36" height="36" viewBox="0 0 36 36" aria-hidden="true">
			<circle class="stroke-n2" cx="18" cy="18" r={RADIUS} fill="none" stroke-width="3.5" />
			<circle
				class={{ accent: 'stroke-accent', warn: 'stroke-warn', danger: 'stroke-danger' }[
					usageTone(share)
				]}
				cx="18"
				cy="18"
				r={RADIUS}
				fill="none"
				stroke-width="3.5"
				stroke-linecap="round"
				stroke-dasharray={CIRCUMFERENCE}
				stroke-dashoffset={CIRCUMFERENCE * (1 - filled)}
			/>
		</svg>
		<span
			class="absolute inset-0 flex items-center justify-center text-3xs text-fg-muted tabular-nums"
		>
			{usagePercent(share)}
		</span>
	</div>
	<div class="flex flex-col">
		<span class="text-xs text-fg-muted">{label}</span>
		{#if resets}
			<span class="text-2xs text-fg-subtle tabular-nums">{resets}</span>
		{/if}
	</div>
</div>
