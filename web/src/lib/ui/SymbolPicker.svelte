<!--
	A task folder's or saved view's icon as a button that opens every list symbol and
	"No symbol". Picking one closes it.
-->
<script lang="ts">
	import type { Component } from 'svelte';
	import type { IntelligenceAgentColor, ListSymbol } from '$lib/protocol/types';
	import { options, vocabulary } from '$lib/protocol/vocabulary.svelte';
	import ListIcon from '$lib/ui/ListIcon.svelte';
	import Popover from '$lib/ui/Popover.svelte';

	interface Props {
		symbol: ListSymbol | null;
		/** The icon shown when there's no symbol. */
		fallback: Component;
		color?: IntelligenceAgentColor | null;
		/** What the symbol is of, for the control's accessible name: "Symbol of Errands". */
		label: string;
		onchange: (symbol: ListSymbol | null) => void;
	}

	let { symbol, fallback, color = null, label, onchange }: Props = $props();

	let open = $state(false);

	function pick(next: ListSymbol | null) {
		open = false;
		if (next !== symbol) onchange(next);
	}

	const choice =
		'inline-flex cursor-pointer items-center justify-center rounded-sm text-fg-muted hover:bg-bg-hover hover:text-fg aria-pressed:bg-current-item aria-pressed:text-fg';
</script>

<Popover bind:open inDialog>
	{#snippet trigger(props)}
		<button
			class="ml-0.5 inline-flex size-6 flex-none cursor-pointer items-center justify-center rounded-sm hover:bg-current-item data-[state=open]:bg-current-item"
			type="button"
			aria-label={label}
			{...props}
		>
			<ListIcon {symbol} {fallback} {color} />
		</button>
	{/snippet}
	<div class="flex flex-col gap-1" role="group" aria-label={label}>
		<button
			class={[choice, 'h-8 justify-start px-2 text-sm']}
			type="button"
			aria-pressed={symbol === null}
			onclick={() => pick(null)}
		>
			No symbol
		</button>
		<div class="grid grid-cols-8 gap-1">
			{#each options(vocabulary.value.list_symbols) as option (option.value)}
				<button
					class={[choice, 'size-8']}
					type="button"
					aria-label={option.label}
					aria-pressed={option.value === symbol}
					onclick={() => pick(option.value)}
				>
					<ListIcon symbol={option.value} {fallback} size={16} quiet="text-current" />
				</button>
			{/each}
		</div>
	</div>
</Popover>
