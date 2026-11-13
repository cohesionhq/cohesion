<!--
	A task folder's or saved view's icon: its symbol, or `fallback` (a folder, a view's filter)
	when it has none, in its color when it has one and quiet grey otherwise.
-->
<script lang="ts">
	import type { Component } from 'svelte';
	import type { ClassValue } from 'svelte/elements';
	import type { IntelligenceAgentColor, ListSymbol } from '$lib/protocol/types';
	import { agentText } from '$lib/ui/colors';
	import { listSymbolIcon } from '$lib/ui/listSymbols';

	interface Props {
		symbol: ListSymbol | null;
		/** Drawn when there's no symbol. */
		fallback: Component;
		color?: IntelligenceAgentColor | null;
		size?: number;
		/** The grey an uncolored icon takes. */
		quiet?: string;
		class?: ClassValue;
	}

	let {
		symbol,
		fallback,
		color = null,
		size = 14,
		quiet = 'text-fg-subtle',
		class: className
	}: Props = $props();

	const Icon = $derived(symbol ? listSymbolIcon[symbol] : fallback);
</script>

<Icon
	class={['flex-none', color ? agentText[color] : quiet, className]}
	{size}
	strokeWidth={1.75}
	aria-hidden="true"
/>
