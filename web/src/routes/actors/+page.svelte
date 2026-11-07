<!--
	The Actors page: a composer to hand an actor a new session, front and
	centre, under it how much of its plan's limits each actor has used, then
	every session, whichever actor has it. No header: until a client offers an
	actor, the page is only a note saying so.
-->
<script lang="ts">
	import SquareTerminal from '@lucide/svelte/icons/square-terminal';
	import { untrack } from 'svelte';
	import ActorUsageList from '$lib/features/actors/ActorUsageList.svelte';
	import SessionList from '$lib/features/actors/SessionList.svelte';
	import NewSession from '$lib/features/actors/NewSession.svelte';
	import { clients } from '$lib/stores/settings/clients.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { actors } = pageStores();

	$effect(() => {
		untrack(() => void Promise.all([actors.load(), actors.usage.load(), clients.load()]));
		// The actors stay loaded app-wide, for the agents' actor sessions beside every page.
		return () => {
			actors.usage.release();
			clients.release();
		};
	});

	const online = (id: number) => clients.isOnline(id);
</script>

<PageTitle title="Actors" />

<div class="flex min-h-full flex-col px-(--page-inset) pt-8 pb-8 max-sm:pt-5">
	<ResourceState resource={actors.list}>
		{#snippet loaded(list)}
			{#if list.length === 0}
				<div class="flex flex-1 flex-col items-center justify-center gap-2 text-center">
					<SquareTerminal size={32} class="mb-2 text-fg-subtle" aria-hidden="true" />
					<h1 class="text-lg">No actors ready</h1>
					<p class="text-fg-muted">
						Clients can offer coding agents or other AI tools to Cohesion.
					</p>
				</div>
			{:else}
				<h1 class="sr-only">Actors</h1>
				<div class="mx-auto flex w-full max-w-180 flex-col gap-12">
					<NewSession actors={list} {online} />
					<ActorUsageList actors={list} usage={actors.usage.value} />
					<SessionList actors={list} sessions={actors.sessions.value} />
				</div>
			{/if}
		{/snippet}
	</ResourceState>
</div>
