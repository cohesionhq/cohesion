<!--
	The Installed tab: each installed extension as a row leading to its page,
	with an Update button when a registry lists a higher version, and a way to
	check registries for updates now rather than at the daily check.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { ExtensionSummary, RegistryResult } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { extensions } from '$lib/stores/extensions.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import InstalledRow from './InstalledRow.svelte';
	import ReviewExtensionDialog from './ReviewExtensionDialog.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	let updating = $state<RegistryResult | null>(null);
	let updateOpen = $state(false);
	let checking = $state(false);

	$effect(() => {
		untrack(() => void extensions.load());
		return () => extensions.list.release();
	});

	function updateFor(extension: ExtensionSummary): RegistryResult | undefined {
		return extensions.updates.value.find((u) => u.extension_id === extension.id)?.result;
	}

	async function checkUpdates() {
		checking = true;
		try {
			await extensions.checkUpdates();
			if (extensions.updates.value.length === 0) toasts.show('Everything is up to date.', 'info');
		} catch (e) {
			toasts.error(e);
		} finally {
			checking = false;
		}
	}
</script>

<ResourceState resource={extensions.list}>
	{#snippet loaded(installed)}
		{#if installed.length === 0}
			<EmptyState
				message="No extensions installed yet"
				actionLabel="Browse extensions"
				onaction={() => void page.goto(routes.extensionsBrowse())}
			/>
		{:else}
			<!-- The tabs' rule already tops the list. -->
			<ul>
				{#each installed as extension (extension.id)}
					<InstalledRow
						{extension}
						update={updateFor(extension)}
						onupdate={(update) => {
							updating = update;
							updateOpen = true;
						}}
					/>
				{/each}
			</ul>
			<p class={[list.note, 'mt-3']}>
				Registries are checked for updates once a day.
				<button
					class="cursor-pointer text-accent hover:underline disabled:cursor-default disabled:text-fg-subtle disabled:no-underline"
					type="button"
					onclick={checkUpdates}
					disabled={checking}>{checking ? 'Checking…' : 'Check now'}</button
				>
			</p>
		{/if}
	{/snippet}
</ResourceState>

<ReviewExtensionDialog bind:open={updateOpen} result={updating} />
