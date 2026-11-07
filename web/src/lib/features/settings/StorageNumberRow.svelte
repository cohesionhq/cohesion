<!--
	One number in Settings › Storage: a label and help, and a field taking a
	whole number of `unit`, saved when it loses focus or on Enter. Anything
	but a whole number of 1 or more puts the saved value back.
-->
<script lang="ts">
	import { list } from '$lib/ui/list';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		label: string;
		help: string;
		/** What the number counts, after the field: "days", "memories". */
		unit: string;
		current: number;
		onsave: (value: number) => Promise<unknown>;
	}

	let { label, help, unit, current, onsave }: Props = $props();

	const saved = $derived(String(current));
	// The typed number, reset to the saved one whenever that changes.
	let value = $derived(saved);
	let busy = $state(false);

	async function commit() {
		const typed = value.trim();
		if (typed === saved) return;
		if (!/^\d+$/.test(typed) || Number(typed) < 1) {
			value = saved;
			toasts.show('Couldn’t save it. Enter a whole number, 1 or more.', 'error');
			return;
		}
		busy = true;
		try {
			await onsave(Number(typed));
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
	<div class={[list.actions, 'max-sm:w-full']}>
		<div class="w-20">
			<TextInput
				class="text-right tabular-nums"
				bind:value
				inputmode="numeric"
				aria-label="{label} in {unit}"
				disabled={busy}
				onchange={() => void commit()}
				onkeydown={(event) => {
					if (event.key === 'Enter') event.currentTarget.blur();
				}}
			/>
		</div>
		<span class="w-18 text-fg-muted">{unit}</span>
	</div>
</li>
