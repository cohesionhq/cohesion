<!--
	One spend period's limit: what model calls cost in it, and the amount it
	may reach before jobs and forks stop calling models. A blank amount, or
	Clear, removes the limit.
-->
<script lang="ts">
	import { usd } from '$lib/format/money';
	import type { IntelligenceSpend } from '$lib/protocol/types';
	import { spend } from '$lib/stores/settings/spend.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { list } from '$lib/ui/list';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		current: IntelligenceSpend;
		label: string;
		help: string;
	}

	let { current, label, help }: Props = $props();

	const saved = $derived(current.limit === null ? '' : current.limit.toFixed(2));
	// The typed amount, reset to the saved one whenever that changes.
	let value = $derived(saved);
	let busy = $state(false);

	async function save(change: () => Promise<void>) {
		busy = true;
		try {
			await change();
		} catch (error) {
			value = saved;
			toasts.error(error);
		} finally {
			busy = false;
		}
	}

	function commit() {
		const typed = value.trim().replace(/^\$/, '');
		if (typed === saved) return;
		if (typed === '') {
			if (current.limit !== null) void save(() => spend.clearLimit(current.period));
			return;
		}
		const amount = Number(typed);
		if (!Number.isFinite(amount) || amount <= 0) {
			value = saved;
			toasts.show('Couldn’t set the limit. Enter an amount more than $0.', 'error');
			return;
		}
		void save(() => spend.setLimit(current.period, amount));
	}
</script>

<li class={list.row}>
	<div class={[list.main, 'flex-col']}>
		<span class="flex items-center gap-2">
			{label}
			{#if current.reached}
				<span
					class="inline-flex h-5 items-center rounded-sm bg-warn/12 px-1.5 text-2xs font-medium text-warn"
					>Reached</span
				>
			{/if}
		</span>
		<span class="text-xs text-fg-subtle">{help}</span>
	</div>
	<div class={[list.actions, 'max-sm:w-full']}>
		<span class="mr-2 text-xs text-fg-muted tabular-nums">
			{usd(current.spent)} spent{#if current.limit !== null}&nbsp;of {usd(current.limit)}{/if}
		</span>
		<div class="relative w-28">
			<span class="pointer-events-none absolute inset-y-0 left-2 flex items-center text-fg-subtle"
				>$</span
			>
			<TextInput
				class="pl-5 tabular-nums"
				bind:value
				inputmode="decimal"
				placeholder="No limit"
				aria-label="{label} spend limit in US dollars"
				disabled={busy}
				onchange={commit}
				onkeydown={(event) => {
					if (event.key === 'Enter') event.currentTarget.blur();
				}}
			/>
		</div>
		<Button
			disabled={busy || current.limit === null}
			onclick={() => void save(() => spend.clearLimit(current.period))}>Clear</Button
		>
	</div>
</li>
