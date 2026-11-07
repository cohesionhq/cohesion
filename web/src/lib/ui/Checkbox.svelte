<!--
	A checkbox, with an optional label after it. `round` draws it as a circle
	that fills with the accent and a check when ticked: a task's, which
	completes it.
-->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import type { Snippet } from 'svelte';
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends Omit<HTMLInputAttributes, 'type'> {
		checked?: boolean;
		round?: boolean;
		children?: Snippet;
	}

	let {
		checked = $bindable(false),
		round = false,
		class: className,
		children,
		...rest
	}: Props = $props();
</script>

<label class="inline-flex cursor-pointer items-center gap-2">
	{#if round}
		<span class="relative inline-grid flex-none place-items-center">
			<input
				class={[
					'peer size-4.5 cursor-pointer appearance-none rounded-full border-[1.5px] border-fg-subtle bg-bg transition-colors checked:border-accent checked:bg-accent hover:border-accent',
					className
				]}
				type="checkbox"
				bind:checked
				{...rest}
			/>
			<Check
				class="pointer-events-none absolute text-accent-fg opacity-0 peer-checked:opacity-100"
				size={11}
				strokeWidth={3.5}
				aria-hidden="true"
			/>
		</span>
	{:else}
		<input class={['size-3.5 accent-accent', className]} type="checkbox" bind:checked {...rest} />
	{/if}
	{#if children}<span>{@render children()}</span>{/if}
</label>
