<!--
	A list to pick one from, for a panel: each row a label with quiet text
	under it, and a check at the trailing edge of the picked one.
-->
<script lang="ts" module>
	export interface Choice {
		id: string | number;
		label: string;
		/** Quiet text under the label (e.g. why it can't be picked). */
		detail?: string;
		disabled?: boolean;
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';

	interface Props {
		/** What the list picks, for its accessible label. */
		label: string;
		choices: Choice[];
		checked: Choice['id'] | null;
		onpick: (id: Choice['id']) => void;
	}

	let { label, choices, checked, onpick }: Props = $props();
</script>

<div class="flex max-w-72 min-w-50 flex-col" role="radiogroup" aria-label={label}>
	{#each choices as choice (choice.id)}
		<button
			class="flex w-full cursor-pointer items-start gap-2 rounded-sm px-2 py-1 text-left text-fg focus-visible:bg-bg-hover focus-visible:outline-none enabled:hover:bg-bg-hover disabled:cursor-default disabled:opacity-50"
			type="button"
			role="radio"
			aria-checked={choice.id === checked}
			disabled={choice.disabled}
			onclick={() => onpick(choice.id)}
		>
			<span class="flex min-w-0 flex-col">
				<span class="truncate">{choice.label}</span>
				{#if choice.detail}<span class="text-xs text-fg-muted">{choice.detail}</span>{/if}
			</span>
			<span class="ml-auto flex h-5 w-3.5 flex-none items-center">
				{#if choice.id === checked}<Check size={14} strokeWidth={2} aria-hidden="true" />{/if}
			</span>
		</button>
	{/each}
</div>
