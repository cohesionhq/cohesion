<!--
	A list's filter field: a magnifier inset at its leading edge, then the
	field, so every list that narrows itself by typing shows the same one.
-->
<script lang="ts">
	import Search from '@lucide/svelte/icons/search';
	import TextInput from './TextInput.svelte';

	interface Props {
		value: string;
		placeholder: string;
		/** Names the field for assistive tech: "Filter documents by title". */
		label: string;
	}

	let { value = $bindable(), placeholder, label }: Props = $props();
</script>

<div class="relative max-w-80">
	<Search
		class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-fg-subtle"
		size={14}
		strokeWidth={1.75}
		aria-hidden="true"
	/>
	<TextInput
		class="pl-8!"
		bind:value
		type="search"
		{placeholder}
		aria-label={label}
		autocomplete="off"
	/>
</div>
