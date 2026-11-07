<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';

	interface Props extends HTMLTextareaAttributes {
		value?: string;
		/** The element, for focus and sizing: `bind:ref`. */
		ref?: HTMLTextAreaElement;
	}

	let {
		value = $bindable(''),
		ref = $bindable(),
		rows = 4,
		class: className,
		...rest
	}: Props = $props();
</script>

<textarea
	class={[
		'w-full resize-y rounded-sm border border-border-strong bg-bg p-2 leading-(--leading) text-fg focus:border-accent focus:shadow-focus focus:outline-none',
		className
	]}
	{rows}
	bind:value
	bind:this={ref}
	{...rest}></textarea>
