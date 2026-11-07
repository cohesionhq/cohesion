<!-- A half with no page to show: its header's controls, over a message. -->
<script lang="ts">
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { paneChrome } from '$lib/ui/paneChrome';

	interface Props {
		message: string;
		actionLabel?: string;
		onaction?: () => void;
	}

	let { message, actionLabel, onaction }: Props = $props();

	const controls = paneChrome()?.controls;
</script>

<div class="flex min-h-full flex-col px-(--page-inset)">
	<div class="box-content flex min-h-8 items-center justify-end pt-3">{@render controls?.()}</div>
	<EmptyState {message} {actionLabel} {onaction} centered />
</div>
