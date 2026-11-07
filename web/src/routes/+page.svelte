<script lang="ts">
	import { onMount } from 'svelte';
	import { getClientInfo } from '$lib/protocol/client';
	import { mockMode } from '$lib/protocol/api';
	import type { ClientInfo } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';

	const page = pane();

	let client = $state<ClientInfo | null>(null);
	let empty = $state(false);
	let creating = $state(false);
	let title = $state('');

	onMount(async () => {
		void workspaces.load();
		client = mockMode ? { workspace: null, target: '' } : await getClientInfo();
	});

	// Land once the list arrives, which may only be after "Try again".
	$effect(() => {
		if (client === null || !workspaces.list.loaded) return;
		const id = workspaces.pick(client.workspace);
		if (id === null) empty = true;
		else void page.goto(routes.workspace(id), { replaceState: true });
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		try {
			const created = await workspaces.create(title.trim());
			creating = false;
			void page.goto(routes.workspace(created.id), { replaceState: true });
		} catch (error) {
			toasts.error(error);
		}
	}
</script>

<PageTitle title="Cohesion" />

{#if empty}
	<div class="px-(--page-inset) pt-12">
		<EmptyState
			message="No workspaces yet"
			actionLabel="Create a workspace"
			primary
			onaction={() => (creating = true)}
		/>
	</div>
{:else if workspaces.list.error}
	<div class="px-(--page-inset) pt-12">
		<EmptyState
			message={workspaces.list.error.message}
			actionLabel="Try again"
			onaction={() => workspaces.list.refetch()}
		/>
	</div>
{/if}

<Dialog bind:open={creating} title="New workspace">
	<form id="first-workspace" onsubmit={create}>
		<Field label="Title" for="first-workspace-title">
			<TextInput id="first-workspace-title" bind:value={title} autocomplete="off" required />
		</Field>
	</form>
	{#snippet footer()}
		<Button onclick={() => (creating = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="first-workspace" disabled={title.trim() === ''}>
			Create
		</Button>
	{/snippet}
</Dialog>
