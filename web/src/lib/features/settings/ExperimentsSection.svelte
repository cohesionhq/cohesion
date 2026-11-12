<!--
	Settings › Experiments: features still being tried, each on or off for
	this Cohesion. A switch applies as it's flipped.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { experiments } from '$lib/stores/settings/experiments.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import Section from './Section.svelte';

	let busy = $state(false);

	$effect(() => {
		untrack(() => void experiments.load());
		return () => experiments.release();
	});

	async function flip(key: string, enabled: boolean) {
		busy = true;
		try {
			await experiments.set(key, enabled);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<ResourceState resource={experiments.list}>
	{#snippet loaded()}
		<Section title="Experiments" description="Try features that are still being worked on.">
			{#if experiments.list.value.length === 0}
				<EmptyState message="No experiments right now" compact />
			{:else}
				<ul class={list.rows}>
					{#each experiments.list.value as experiment (experiment.key)}
						<li class={list.row}>
							<div class={[list.main, 'flex-col']}>
								<span>{experiment.title}</span>
								<span class="text-xs text-fg-subtle">{experiment.description}</span>
							</div>
							<div class={[list.actions, 'justify-end']}>
								<Switch
									checked={experiment.enabled}
									label={experiment.title}
									disabled={busy}
									onchange={(checked) => void flip(experiment.key, checked)}
								/>
							</div>
						</li>
					{/each}
				</ul>
			{/if}
		</Section>
	{/snippet}
</ResourceState>
