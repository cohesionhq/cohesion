<!--
	A resource's panel (`ItemPanel`): its kind, the account it came from and
	when it last changed, then its text, with Open going to its page.
-->
<script lang="ts">
	import Cable from '@lucide/svelte/icons/cable';
	import Clock from '@lucide/svelte/icons/clock';
	import Layers from '@lucide/svelte/icons/layers';
	import Package from '@lucide/svelte/icons/package';
	import { untrack } from 'svelte';
	import { ago } from '$lib/format/dates';
	import type { Resource } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { connectors } from '$lib/stores/connectors.svelte';
	import ItemDetail from '$lib/ui/ItemDetail.svelte';
	import ItemPanel from '$lib/ui/ItemPanel.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';

	let { open = $bindable(false), resource }: { open?: boolean; resource: Resource } = $props();

	const account = $derived(connectors.account(resource.account_id));

	$effect(() => {
		untrack(() => void connectors.load());
	});
</script>

<ItemPanel
	bind:open
	title={resource.title}
	icon={Package}
	kind={connectors.kindTitle(resource)}
	href={routes.resource(resource.workspace_id, resource.kind_id, resource.id)}
	link={{ kind: 'resource', ws: resource.workspace_id, id: resource.id }}
>
	{#snippet details()}
		<ItemDetail label="Kind" icon={Layers}>{connectors.kindTitle(resource)}</ItemDetail>
		<ItemDetail label="Account" icon={Cable}
			>{account ? `${account.connector.title}, ${account.description}` : 'Removed'}</ItemDetail
		>
		<ItemDetail label="Updated" icon={Clock}>
			{resource.last_updated > 0 ? ago(resource.last_updated) : 'Never'}
		</ItemDetail>
	{/snippet}
	{#if resource.contents.trim() === ''}
		<p class="text-fg-subtle">Nothing in it.</p>
	{:else if resource.content_kind === 'json'}
		<pre class="overflow-x-auto font-mono text-xs whitespace-pre-wrap">{resource.contents}</pre>
	{:else}
		<div class="text-md"><Markdown source={resource.contents} /></div>
	{/if}
</ItemPanel>
