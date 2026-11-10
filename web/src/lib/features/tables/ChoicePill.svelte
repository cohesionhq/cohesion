<!--
	A choice's option as a small pill, drawn as a tag is: its name on a faint
	wash of its color, or in a tag's grey when it has none. Cut short when it
	is wider than its cell.
-->
<script lang="ts">
	import type { PropertyOption } from '$lib/protocol/types';
	import { agentChip } from '$lib/ui/colors';

	let { option }: { option: Pick<PropertyOption, 'name' | 'color'> } = $props();
</script>

<span
	class={[
		'inline-flex h-4.5 max-w-full min-w-0 flex-none items-center rounded-full px-1.75 text-2xs leading-none font-medium',
		option.color ? agentChip[option.color] : 'bg-current-item text-fg-muted'
	]}><span class="truncate">{option.name}</span></span
>
