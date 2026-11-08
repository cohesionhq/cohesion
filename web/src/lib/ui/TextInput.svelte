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
		'h-7.5 w-full rounded-sm border border-border bg-bg px-2 text-fg transition-colors placeholder:text-fg-subtle hover:border-border-strong focus:border-accent focus:shadow-focus focus:outline-none',
		className
	]}
	{type}
	bind:value
	bind:this={element}
	{...rest}
/>
