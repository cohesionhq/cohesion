<!--
	Installing from a URL: Cohesion downloads the script and reads its
	manifest, and the review opens on it just as for a file.
-->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ExtensionPreview } from '$lib/protocol/types';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		/** Called with the daemon's reading of the downloaded script, to review before installing. */
		onreview: (file: { name: string; preview: ExtensionPreview }) => void;
	}

	let { open = $bindable(false), onreview }: Props = $props();

	const id = $props.id();
	let url = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (open) {
			url = '';
			error = null;
		}
	});

	async function review(event: SubmitEvent) {
		event.preventDefault();
		const source_url = url.trim();
		busy = true;
		error = null;
		try {
			const preview = await extensions.preview(source_url);
			open = false;
			onreview({ name: source_url, preview });
		} catch (e) {
			const failure = toProtocolError(e);
			// A bad address, a failed download or a script that doesn't read stays in the dialog.
			if (failure.code === 'validationFailed' || failure.code === 'upstreamError')
				error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open
	title="Install from URL"
	description="Cohesion downloads the script, then shows it for review."
>
	<form id="{id}-form" onsubmit={review}>
		<Field label="URL" for="{id}-url" {error}>
			<TextInput
				id="{id}-url"
				type="url"
				bind:value={url}
				placeholder="https://example.com/extension.js"
				autocomplete="off"
				spellcheck="false"
				aria-invalid={error !== null}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="{id}-form" disabled={busy || url.trim() === ''}>
			{busy ? 'Downloading…' : 'Review…'}
		</Button>
	{/snippet}
</Dialog>
