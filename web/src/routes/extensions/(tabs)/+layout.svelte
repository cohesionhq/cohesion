<!--
	Extensions: Installed and Browse as tabs under one header, which also
	holds installing from a file or a URL. An extension's own page draws its own header.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import InstallExtensionDialog from '$lib/features/extensions/InstallExtensionDialog.svelte';
	import InstallFromUrlDialog from '$lib/features/extensions/InstallFromUrlDialog.svelte';
	import ReviewExtensionDialog from '$lib/features/extensions/ReviewExtensionDialog.svelte';
	import type { ExtensionPreview } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import Tabs from '$lib/ui/Tabs.svelte';
	import { pane } from '$lib/shell/pane.svelte';

	const page = pane();

	let { children }: { children: Snippet } = $props();

	let installing = $state(false);
	let installingFromUrl = $state(false);
	let reviewing = $state<{ name: string | null; preview: ExtensionPreview } | null>(null);
	let reviewOpen = $state(false);
</script>

<div class="px-(--page-inset)">
	<PageHeader title="Extensions">
		{#snippet actions()}
			<Button onclick={() => (installingFromUrl = true)}>Install from URL…</Button>
			<Button onclick={() => (installing = true)}>Install from file…</Button>
		{/snippet}
		{#snippet tabs()}
			<Tabs
				label="Extensions"
				tabs={[
					{
						href: routes.extensionsInstalled(),
						label: 'Installed',
						current: page.route.id === '/extensions/(tabs)/installed',
						badge: extensions.updates.value.length
					},
					{
						href: routes.extensionsBrowse(),
						label: 'Browse',
						current: page.route.id === '/extensions/(tabs)/browse'
					}
				]}
			/>
		{/snippet}
	</PageHeader>
	<!-- Moving between tabs fades only this (see the root layout). -->
	<div data-view-page="/extensions/(tabs)">
		{@render children()}
	</div>
</div>

<InstallExtensionDialog
	bind:open={installing}
	onreview={(file) => {
		reviewing = file;
		reviewOpen = true;
	}}
/>
<InstallFromUrlDialog
	bind:open={installingFromUrl}
	onreview={(file) => {
		reviewing = file;
		reviewOpen = true;
	}}
/>
<ReviewExtensionDialog bind:open={reviewOpen} file={reviewing} />
