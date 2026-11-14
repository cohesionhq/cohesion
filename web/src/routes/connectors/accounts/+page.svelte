<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import AccountsSection from '$lib/features/connectors/AccountsSection.svelte';
	import AddAccountDialog from '$lib/features/connectors/AddAccountDialog.svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ConnectorService, RedactedConnectorAccount } from '$lib/protocol/types';
	import { connectors } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import Menu, { type MenuItem } from '$lib/ui/Menu.svelte';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	/** Null until the extensions answer. */
	let services = $state<ConnectorService[] | null>(null);
	let servicesError = $state<string | null>(null);
	let adding = $state<ConnectorService | null>(null);
	let addOpen = $state(false);
	let reauthenticating = $state<RedactedConnectorAccount | null>(null);

	$effect(() => {
		untrack(() => void loadServices());
	});

	async function loadServices() {
		try {
			services = await connectors.services();
			servicesError = null;
		} catch (e) {
			servicesError = toProtocolError(e).message;
		}
	}

	function reauthenticate(account: RedactedConnectorAccount) {
		const extensionId = account.host.kind === 'extension' ? account.host.extension_id : null;
		const service = services?.find((s) => s.extension_id === extensionId);
		if (!service || service.error !== null) {
			toasts.show(
				servicesError ?? service?.error ?? 'This account’s service isn’t ready yet. Try again.',
				'error'
			);
			return;
		}
		reauthenticating = account;
		adding = service;
		addOpen = true;
	}

	const items = $derived<MenuItem[]>(
		(services ?? [])
			.toSorted((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }))
			.map((service) => ({
				id: service.extension_id,
				label: service.title,
				detail: service.error ?? undefined,
				disabled: service.error !== null,
				onSelect: () => {
					reauthenticating = null;
					adding = service;
					addOpen = true;
				}
			}))
	);
</script>

<PageTitle title="Accounts" />

<div class="px-(--page-inset)">
	<PageHeader title="Accounts">
		{#snippet actions()}
			<Menu {items}>
				{#snippet trigger(props)}
					<Button variant="secondary" {...props}
						><Plus size={15} strokeWidth={1.75} aria-hidden="true" />Add account</Button
					>
				{/snippet}
				{#snippet header()}
					{#if servicesError !== null}
						{servicesError}
					{:else if services === null}
						Loading services…
					{:else if services.length === 0}
						No services to sign in to. Install an extension that connects one.
					{:else}
						Choose a service to sign in to
					{/if}
				{/snippet}
				{#snippet icon(item)}
					{@const service = services?.find((s) => s.extension_id === item.id)}
					{#if service}<IconTile icon={service.icon} title={service.title} />{/if}
				{/snippet}
			</Menu>
		{/snippet}
	</PageHeader>
	<AccountsSection onreauthenticate={reauthenticate} />
</div>

<AddAccountDialog bind:open={addOpen} service={adding} account={reauthenticating} />
