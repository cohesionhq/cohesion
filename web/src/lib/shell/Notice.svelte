<script lang="ts">
	interface Props {
		message: string;
		code?: string;
	}

	let { message, code }: Props = $props();
</script>

<main class="grid min-h-screen place-items-center p-(--gutter) text-center text-md text-fg-muted">
	<p>
		{message}
		{#if code}<code class="rounded-sm border border-border px-1 py-px text-fg">{code}</code>.{/if}
	</p>
</main>
