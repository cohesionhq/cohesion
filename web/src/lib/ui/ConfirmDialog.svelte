<!--
	A destructive action's confirmation. `onconfirm` runs the action; the
	dialog closes when it resolves. It opens for `open`, or while `subject`
	(the row being acted on, handed to `onconfirm`) is set, in which case
	`onclose` clears it once the dialog has faded out, so the title doesn't
	change as it goes.
-->
<script lang="ts" generics="T">
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		subject?: T | null;
		onclose?: () => void;
		title: string;
		description?: string;
		confirmLabel?: string;
		onconfirm: (subject: T) => Promise<unknown>;
	}

	let {
		open = $bindable(false),
		subject = null,
		onclose,
		title,
		description,
		confirmLabel = 'Delete',
		onconfirm
	}: Props = $props();

	let busy = $state(false);
	/** Set from closing until the fade ends, while `subject` is still set. */
	let closing = $state(false);

	function close() {
		open = false;
		closing = true;
	}

	async function confirm() {
		busy = true;
		try {
			await onconfirm(subject as T);
			close();
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open={
		() => (open || subject !== null) && !closing,
		(value) => {
			if (!value) close();
		}
	}
	{title}
	{description}
	onclosed={() => {
		closing = false;
		onclose?.();
	}}
>
	{#snippet footer()}
		<Button onclick={close}>Cancel</Button>
		<Button variant="danger-quiet" onclick={confirm} disabled={busy}>{confirmLabel}</Button>
	{/snippet}
</Dialog>
