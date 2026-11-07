<!-- Asks for a feed's address, follows it and opens it. -->
<script lang="ts">
	import { toProtocolError } from '$lib/protocol/errors';
	import { routes } from '$lib/shell/routes';
	import { viewParam } from '$lib/stores/articles.svelte';
	import { feeds } from '$lib/stores/feeds.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	interface Props {
		open?: boolean;
		workspaceId: number;
	}

	let { open = $bindable(false), workspaceId }: Props = $props();

	let url = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (url.trim() === '') return;
		busy = true;
		try {
			const feed = await feeds.add(url.trim());
			open = false;
			toasts.show(`Added “${feed.title}”.`);
			void page.goto(routes.articles(workspaceId, viewParam({ feed: feed.id })));
		} catch (e) {
			const protocolError = toProtocolError(e);
			if (protocolError.code === 'validationFailed') error = protocolError.message;
			else toasts.error(protocolError);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open
	title="Add feed"
	description="Cohesion reads this feed now and every hour. Articles from this feed show in every workspace."
	onclosed={() => {
		url = '';
		error = null;
	}}
>
	<form id="add-feed-form" onsubmit={submit}>
		<Field label="Address" for="add-feed-url" {error}>
			<TextInput
				id="add-feed-url"
				type="url"
				placeholder="https://example.com/feed.xml"
				bind:value={url}
				autocomplete="off"
				oninput={() => (error = null)}
			/>
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="add-feed-form"
			disabled={busy || url.trim() === ''}
		>
			{busy ? 'Adding…' : 'Add'}
		</Button>
	{/snippet}
</Dialog>
