<!--
	A form's optional fields, folded under a quiet summary so the form opens
	on what it needs. It starts closed, even when a field inside is set; the
	form opens it when a field inside has an error, and it opens itself when
	the browser refuses a field inside, so the field it points at shows.
-->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { Snippet } from 'svelte';
	import type { ClassValue } from 'svelte/elements';

	interface Props {
		label: string;
		open?: boolean;
		/** Classes for the fields' column, to match the form's spacing. */
		class?: ClassValue;
		children: Snippet;
	}

	let { label, open = $bindable(false), class: className, children }: Props = $props();
</script>

<!-- Opened at once, not through `open`, so the field is showing when the browser focuses it. -->
<details class="group" bind:open oninvalidcapture={(event) => (event.currentTarget.open = true)}>
	<summary
		class="flex w-fit cursor-pointer items-center gap-1.5 rounded-sm text-xs font-medium text-fg-subtle select-none hover:text-fg focus-visible:shadow-focus focus-visible:outline-none [&::-webkit-details-marker]:hidden"
	>
		<ChevronRight
			class="flex-none transition-transform group-open:rotate-90"
			size={14}
			strokeWidth={1.75}
			aria-hidden="true"
		/>
		{label}
	</summary>
	<div class={['flex flex-col', className]}>
		{@render children()}
	</div>
</details>
