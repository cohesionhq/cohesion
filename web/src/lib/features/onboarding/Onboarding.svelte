<!--
	The first-run flow, shown while there are no workspaces: a workspace, a
	model provider and an agent, one page each and each skippable, under a
	bar showing how close the setup is to working. Escape skips the rest.
	`onfinish` gets the workspace made here, if one was.
-->
<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { untrack } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { IntelligenceAgent, Workspace } from '$lib/protocol/types';
	import { pageStores } from '$lib/stores/pageStores.svelte';
	import { intelligence } from '$lib/stores/settings/intelligence.svelte';
	import { motion } from '$lib/ui/motion';
	import AgentStep from './AgentStep.svelte';
	import DoneStep from './DoneStep.svelte';
	import IntelligenceStep from './IntelligenceStep.svelte';
	import { nextPage, previousPage, type Page, type Step } from './onboarding';
	import Progress from './Progress.svelte';
	import WorkspaceStep from './WorkspaceStep.svelte';

	interface Props {
		open?: boolean;
	}

	let { open = $bindable(false) }: Props = $props();

	const { agents } = pageStores();

	let page = $state<Page>('workspace');
	let workspace = $state<Workspace | null>(null);
	let agent = $state<IntelligenceAgent | null>(null);

	// What is already set up counts, so a provider or agent kept from before shows as done.
	const done = $derived(
		new Set<Step>([
			...(workspace ? (['workspace'] as const) : []),
			...(intelligence.endpoints.value.length > 0 ? (['intelligence'] as const) : []),
			...(agent || agents.list.value.length > 0 ? (['agent'] as const) : [])
		])
	);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			page = 'workspace';
			workspace = null;
			agent = null;
			void intelligence.endpoints.load();
			void agents.loadList();
		});
		return () => intelligence.release();
	});

	const next = () => (page = nextPage(page));
	const back = () => (page = previousPage(page) ?? page);
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="overlay-backdrop" />
		<Dialog.Content
			interactOutsideBehavior="ignore"
			class="overlay-surface fixed top-[calc(50%+var(--titlebar-inset)/2)] left-1/2 z-50 flex h-[min(760px,calc(100dvh-2*var(--dialog-inset)-var(--titlebar-inset)))] w-[min(420px,calc(100vw-2*var(--gutter)))] -translate-1/2 flex-col gap-6 overflow-y-auto p-8 max-sm:gap-6 max-sm:p-5"
		>
			<Progress {done} {page} />
			{#key page}
				<div class="flex flex-1 flex-col" in:fade={{ duration: motion.duration }}>
					{#if page === 'workspace'}
						<WorkspaceStep
							created={workspace}
							oncreated={(created) => (workspace = created)}
							onnext={next}
						/>
					{:else if page === 'intelligence'}
						<IntelligenceStep onback={back} onnext={next} />
					{:else if page === 'agent'}
						<AgentStep
							created={agent}
							oncreated={(created) => (agent = created)}
							onback={back}
							onnext={next}
						/>
					{:else}
						<DoneStep {done} {workspace} onback={back} onfinish={() => (open = false)} />
					{/if}
				</div>
			{/key}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
