<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends HTMLInputAttributes {
		value?: string;
		element?: HTMLInputElement;
	}

	let {
		value = $bindable(''),
		element = $bindable(),
		type = 'text',
		class: className,
		...rest
	}: Props = $props();
</script>

<input
	class={[
		'h-8 w-full rounded-sm border border-border-strong bg-bg px-2 text-fg placeholder:text-fg-subtle focus:border-accent focus:shadow-focus focus:outline-none',
		className
	]}
	{type}
	bind:value
	bind:this={element}
	{...rest}
/>
