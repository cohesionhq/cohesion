<!--
	A thin bar filled as far as a share of a limit is used, in the accent,
	then the warning color past 80%, the danger color past 95%.
-->
<script lang="ts">
	import { usagePercent, usageTone } from './plan';

	let { share }: { share: number } = $props();
</script>

<div class="h-1.5 overflow-hidden rounded-full bg-n3">
	<div
		class={[
			'h-full rounded-full',
			{ accent: 'bg-accent', warn: 'bg-warn', danger: 'bg-danger' }[usageTone(share)]
		]}
		style:width={usagePercent(Math.min(share, 1))}
	></div>
</div>
