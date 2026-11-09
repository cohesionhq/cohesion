<script lang="ts">
	import { untrack } from 'svelte';
	import type { Resource as ImportedResource } from '$lib/protocol/types';
	import { ago } from '$lib/format/dates';
	import { routes } from '$lib/shell/routes';
	import { connectors } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import CohesionUI from '$lib/ui/CohesionUI.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { pageStores } from '$lib/stores/pageStores.svelte';

	const { resources } = pageStores();

	const page = pane();

	const ws = $derived(Number(page.params.ws));
	const kindId = $derived(Number(page.params.kind));
	const id = $derived(Number(page.params.id));
	const resource = $derived(resources.current.value);
	/** The connector's own view of it; without one, its contents show. */
	const view = $derived(resources.view.value);
	const account = $derived(resource ? connectors.account(resource.account_id) : undefined);
	/** Where back goes: the kind's page, named once its connector loads. */
	const back = $derived({
		href: routes.resources(ws, kindId),
		label: connectors.kind(kindId)?.kind.plural ?? 'Resources'
	});
	let removing = $state(false);

	$effect(() => {
		untrack(() => void connectors.load());
	});

	$effect(() => {
		const [w, a] = [ws, id];
		untrack(() => void resources.open(w, a));
		return () => resources.close();
	});

	/** JSON contents, indented; as stored if they don't parse. */
	function pretty(contents: string): string {
		try {
			return JSON.stringify(JSON.parse(contents), null, 2);
		} catch {
			return contents;
		}
	}

	async function remove() {
		const { href } = back;
		await resources.remove(ws, id);
		void page.goto(href, { replaceState: true });
	}
</script>

<PageTitle title={`${resource ? `${resource.title} · $${back.label}` : back.label}`} />

<div class="px-(--page-inset) pb-(--page-inset)">
	<ResourceState
		resource={resources.current}
		missing="Resource not found"
		missingAction="Back to {back.label.toLowerCase()}"
		onmissing={() => void page.goto(back.href, { replaceState: true })}
	>
		{#snippet loaded(resource)}
			<PageHeader title={resource.title} {back}>
				{#snippet meta()}
					<span class="inline-flex items-center gap-1.5">
						{#if account}
							<IconTile icon={account.connector.icon} title={account.connector.title} size={14} />
						{/if}
						{[
							connectors.kindTitle(resource),
							resource.last_updated > 0 ? `updated ${ago(resource.last_updated)}` : '',
							account ? `via ${account.connector.title}` : ''
						]
							.filter(Boolean)
							.join(' · ')}
					</span>
				{/snippet}
				{#snippet actions()}
					<Button variant="danger-quiet" onclick={() => (removing = true)}>Remove…</Button>
				{/snippet}
			</PageHeader>

			{#if view}
				<CohesionUI component={view} />
			{:else if !resources.view.loaded && !resources.view.error}
				<!-- Waits for the connector's view, so the contents don't flash first. -->
			{:else}
				{#if resources.view.error}
					<p class="mb-3 text-xs text-fg-muted">{resources.view.error.message}</p>
				{/if}
				{@render contents(resource)}
			{/if}
		{/snippet}
	</ResourceState>
</div>

{#snippet contents(resource: ImportedResource)}
	{#if resource.content_kind === 'markdown'}
		<Markdown source={resource.contents} />
	{:else}
		<pre
			class="overflow-x-auto overscroll-y-auto rounded-sm border border-border bg-bg-raised p-3 font-mono text-xs">{pretty(
				resource.contents
			)}</pre>
	{/if}
{/snippet}

<ConfirmDialog
	bind:open={removing}
	title="Remove “{resource?.title ?? ''}”?"
	description="This resource will be removed from the workspace. To get it back, import it again."
	confirmLabel="Remove"
	onconfirm={remove}
/>
