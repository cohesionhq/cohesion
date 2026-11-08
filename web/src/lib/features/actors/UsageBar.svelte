<!--
	A thin bar filled as far as a share of a limit is used, in a neutral grey,
	then the warning color past 80%, the danger color past 95%: color only
	once it means something.
-->
<script lang="ts">
	import { usagePercent, usageTone } from './plan';

	let { share }: { share: number } = $props();
</script>

<!-- The track a tint of the text, so it reads on the canvas as on the sheet. -->
<div class="h-1.5 overflow-hidden rounded-full bg-fg/10">
	<div
		class={[
			'h-full rounded-full',
			{ calm: 'bg-fg-subtle', warn: 'bg-warn', danger: 'bg-danger' }[usageTone(share)]
		]}
		style:width={usagePercent(Math.min(share, 1))}
	></div>
</div>
