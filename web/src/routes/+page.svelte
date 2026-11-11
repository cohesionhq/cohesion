<script lang="ts">
	import { onMount } from 'svelte';
	import { getClientInfo } from '$lib/protocol/client';
	import { mockMode } from '$lib/protocol/api';
	import type { ClientInfo } from '$lib/protocol/types';
	import Onboarding from '$lib/features/onboarding/Onboarding.svelte';
	import { routes } from '$lib/shell/routes';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';

	const page = pane();

	let client = $state<ClientInfo | null>(null);
	let empty = $state(false);
	let onboarding = $state(false);

	onMount(async () => {
		void workspaces.load();
		client = mockMode ? { workspace: null, target: '' } : await getClientInfo();
	});

	// Land once the list arrives, which may only be after "Try again". With no workspace, the
	// first-run flow opens once; a workspace made there is landed in when it closes.
	$effect(() => {
		if (client === null || !workspaces.list.loaded || onboarding) return;
		const id = workspaces.pick(client.workspace);
		if (id !== null) void page.goto(routes.workspace(id), { replaceState: true });
		else if (!empty) {
			empty = true;
			onboarding = true;
		}
	});
</script>

<PageTitle title="Cohesion" />

{#if empty}
	<div class="px-(--page-inset) pt-12">
		<EmptyState
			message="No workspaces yet"
			actionLabel="Create a workspace"
			primary
			onaction={() => (onboarding = true)}
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

<Onboarding bind:open={onboarding} />
