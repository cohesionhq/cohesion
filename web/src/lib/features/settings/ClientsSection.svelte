<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import { ago, shortDate } from '$lib/format/dates';
	import type { Client, ListedClient } from '$lib/protocol/types';
	import { clients, shortKey } from '$lib/stores/settings/clients.svelte';
	import Button from '$lib/ui/Button.svelte';
	import OnlineBadge from '$lib/ui/OnlineBadge.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import AddClientDialog from './AddClientDialog.svelte';
	import Section from './Section.svelte';

	let adding = $state(false);
	let revoking = $state<Client | null>(null);
	let editingId = $state<number | null>(null);
	let editName = $state('');
	let editError = $state<string | null>(null);
	let editInput = $state<HTMLInputElement>();

	$effect(() => {
		editInput?.focus();
		editInput?.select();
	});

	function startRename(client: Client) {
		editingId = client.id;
		editName = client.client_name;
		editError = null;
	}

	async function saveRename(client: Client) {
		// Enter saves and closes the field; the blur that follows finds it closed.
		if (editingId !== client.id) return;
		const name = editName.trim();
		if (name === client.client_name) {
			editingId = null;
			return;
		}
		try {
			await clients.rename(client.id, name);
			editingId = null;
		} catch (error) {
			const e = toProtocolError(error);
			if (e.code === 'validationFailed') editError = e.message;
			else toasts.error(e);
		}
	}

	function renameKey(event: KeyboardEvent, client: Client) {
		if (event.key === 'Enter') {
			event.preventDefault();
			void saveRename(client);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			editingId = null;
		}
	}

	$effect(() => {
		untrack(() => void clients.load());
		return () => clients.release();
	});

	/** When and where a client last connected, and when it paired. */
	function history(client: ListedClient): string {
		const from = client.last_address ? ` from ${client.last_address}` : '';
		const seen = client.online
			? `Connected${from}`
			: client.last_seen_at
				? `Last connected ${ago(client.last_seen_at)}${from}`
				: null;
		const paired = client.created_at ? `Paired ${shortDate(client.created_at)}` : null;
		return [seen, paired].filter(Boolean).join(' · ');
	}

	/** Revoking a client: it can't connect until it's set up again, and what it offered goes. */
	const revokeDescription =
		'This client won’t be able to connect or offer anything to Cohesion until it’s set up again. If it’s the CLI serving this page, this page will disconnect too. Any actors it offers will also be removed, but their tasks won’t be deleted.';
</script>

<Section
	title="Clients"
	description="These apps and tools are paired with Cohesion. A client can offer actors to hand sessions to, and then sees the prompts and data Cohesion sends it."
>
	{#snippet actions()}
		<Button
			variant="primary"
			disabled={clients.pairing.value.refusal !== null}
			onclick={() => (adding = true)}>Add client…</Button
		>
	{/snippet}
	<ResourceState resource={clients.list}>
		{#snippet loaded()}
			<ul class={list.rows}>
				{#each clients.list.value as client (client.id)}
					<li class={list.row}>
						{#if editingId === client.id}
							<div class="min-w-0 flex-1">
								<TextInput
									bind:value={editName}
									aria-label="Client name"
									autocomplete="off"
									bind:element={editInput}
									onkeydown={(event) => renameKey(event, client)}
									onblur={() => void saveRename(client)}
								/>
								{#if editError}<p class={list.error}>{editError}</p>{/if}
							</div>
						{:else}
							<div class={list.main}>
								<span>{client.client_name}</span>
								<OnlineBadge online={client.online} />
								{#if client.current}<span class="text-xs text-fg-muted">This client</span>{/if}
								{#if client.key_kind === 'certificate'}
									<span class="font-mono text-xs text-fg-subtle">{shortKey(client.key)}</span>
								{:else}
									<span class="text-xs text-fg-subtle">On the same computer as Cohesion</span>
								{/if}
								{#if history(client)}
									<span class="basis-full text-xs text-fg-subtle">{history(client)}</span>
								{/if}
							</div>
							<div class={list.actions}>
								<RowMenu
									label="More for {client.client_name}"
									items={[
										{ label: 'Rename', onSelect: () => startRename(client) },
										{
											label: 'Revoke…',
											danger: true,
											separated: true,
											onSelect: () => (revoking = client)
										}
									]}
								/>
							</div>
						{/if}
					</li>
				{:else}
					<li class={list.empty}>No clients yet.</li>
				{/each}
			</ul>
		{/snippet}
	</ResourceState>
	{#if clients.pairing.value.refusal}
		<p class={list.note}>{clients.pairing.value.refusal}</p>
	{/if}
</Section>

<AddClientDialog bind:open={adding} />

<ConfirmDialog
	subject={revoking}
	onclose={() => (revoking = null)}
	title="Revoke {revoking?.client_name ?? 'client'}?"
	description={revokeDescription}
	confirmLabel="Revoke"
	onconfirm={(client) => clients.revoke(client.id)}
/>
