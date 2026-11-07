<!--
	Whether a client is connected now: a small accent dot and "Online", or a
	grey one and "Offline".
-->
<script lang="ts">
	interface Props {
		online: boolean;
	}

	let { online }: Props = $props();
</script>

<span class="inline-flex flex-none items-center gap-1.5 self-center text-xs text-fg-subtle">
	<span
		class={['size-1.5 rounded-full', online ? 'bg-accent' : 'bg-fg-subtle/50']}
		aria-hidden="true"
	></span>
	{online ? 'Online' : 'Offline'}
</span>
