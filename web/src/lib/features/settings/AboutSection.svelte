<script lang="ts">
	import { mockMode, send } from '$lib/protocol/api';
	import { getClientInfo } from '$lib/protocol/client';
	import { toProtocolError } from '$lib/protocol/errors';
	import { PROTOCOL_VERSION, type AboutResponse, type ClientInfo } from '$lib/protocol/types';
	import { list } from '$lib/ui/list';
	import Section from './Section.svelte';

	let about = $state<AboutResponse | null>(null);
	let aboutError = $state<string | null>(null);
	let client = $state<ClientInfo | null>(null);

	$effect(() => {
		send('about')
			.then((reply) => (about = reply))
			.catch((error) => (aboutError = toProtocolError(error).message));
		(mockMode ? Promise.resolve({ workspace: null, target: 'mock' }) : getClientInfo())
			.then((info) => (client = info))
			.catch(() => undefined);
	});

	const rows = $derived([
		{ label: 'Computer', value: about?.hostname ?? (aboutError ? '—' : '…') },
		{ label: 'Version', value: about?.version ?? (aboutError ? '—' : '…') },
		{ label: 'Protocol', value: String(PROTOCOL_VERSION) },
		{ label: 'CLI target', value: client ? client.target || '—' : '…' }
	]);
</script>

<Section title="About">
	<dl class={list.rows}>
		{#each rows as row (row.label)}
			<div class={list.row}>
				<dt class={[list.main, 'text-fg-muted']}>{row.label}</dt>
				<dd class="font-mono text-xs wrap-anywhere">{row.value}</dd>
			</div>
		{/each}
	</dl>
	{#if aboutError}<p class={list.error}>{aboutError}</p>{/if}
</Section>
