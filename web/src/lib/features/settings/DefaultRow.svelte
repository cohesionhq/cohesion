<!-- One tier's default model: pick one of an endpoint's models to set it, or "Not set" to clear it. -->
<script lang="ts">
	import type { IntelligenceDefault, IntelligenceTier } from '$lib/protocol/types';
	import { intelligence } from '$lib/stores/settings/intelligence.svelte';
	import { list } from '$lib/ui/list';
	import Select, { type SelectGroup } from '$lib/ui/Select.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		tier: IntelligenceTier;
		label: string;
		help: string;
		current: IntelligenceDefault | undefined;
		/** Every endpoint's models, one group per endpoint. */
		groups: SelectGroup[];
	}

	let { tier, label, help, current, groups }: Props = $props();

	const saved = $derived(current ? String(current.endpoint_model_id) : '');
	// The picked value, reset to the saved one whenever that changes.
	let value = $derived(saved);
	let busy = $state(false);

	async function pick() {
		if (value === saved) return;
		busy = true;
		try {
			if (value === '') await intelligence.clearDefault(tier);
			else await intelligence.setDefault(tier, Number(value));
		} catch (error) {
			value = saved;
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<li class={list.row}>
	<div class={[list.main, 'flex-col']}>
		<span>{label}</span>
		<span class="text-xs text-fg-subtle">{help}</span>
	</div>
	<!-- One width for every tier's picker, so they line up down the list. -->
	<div class={[list.actions, 'w-72 max-sm:w-full']}>
		<Select
			class="w-full"
			bind:value
			options={[{ value: '', label: 'Not set' }, ...groups]}
			aria-label="{label} default model"
			disabled={busy}
			onchange={() => void pick()}
		/>
	</div>
</li>
