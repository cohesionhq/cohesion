<!--
	The Accounts page's body: every account the user signed in to through an
	extension granted `oauth`, each with that extension, and removing one;
	and every orphaned account, kept with its resources until they move to
	another account of its connector or it is removed.
-->
<script lang="ts">
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import { untrack } from 'svelte';
	import { flip } from 'svelte/animate';
	import { fade, fly } from 'svelte/transition';
	import type { AccountHost, RedactedConnectorAccount } from '$lib/protocol/types';
	import { connectors } from '$lib/stores/connectors.svelte';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { list } from '$lib/ui/list';
	import type { MenuItem } from '$lib/ui/Menu.svelte';
	import { motion } from '$lib/ui/motion';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import RowMenu from '$lib/ui/RowMenu.svelte';
	import MoveResourcesDialog from './MoveResourcesDialog.svelte';

	interface Props {
		/** Called to sign `account` in again; its extension's sign-in page opens. */
		onreauthenticate: (account: RedactedConnectorAccount) => void;
	}

	let { onreauthenticate }: Props = $props();

	let removing = $state<RedactedConnectorAccount | null>(null);
	let moving = $state<RedactedConnectorAccount | null>(null);

	$effect(() => {
		untrack(() => void Promise.all([connectors.load(), extensions.load()]));
		return () => extensions.list.release();
	});

	function extensionName(host: AccountHost): string | undefined {
		if (host.kind !== 'extension') return undefined;
		return extensions.list.value.find((e) => e.id === host.extension_id)?.name;
	}

	/** What its row's menu offers: signing in again when an extension serves it, moving its resources when it has a sibling, and removing it. */
	function actions(account: RedactedConnectorAccount): MenuItem[] {
		return [
			...(account.host.kind === 'extension'
				? [{ label: 'Sign in again…', onSelect: () => onreauthenticate(account) }]
				: []),
			...(connectors.siblings(account).length > 0
				? [{ label: 'Move resources…', onSelect: () => (moving = account) }]
				: []),
			{ label: 'Remove…', danger: true, onSelect: () => (removing = account) }
		];
	}
</script>

<ResourceState resource={connectors.accounts}>
	{#snippet loaded(accounts)}
		<!-- The page header's rule already tops the list. -->
		<ul>
			{#each accounts as account (account.id)}
				{@const extension = extensionName(account.host)}
				<!-- A removed row fades where it was while the rest flip up into its place:
				     `animate:` lifts a leaving row out of the flow, so collapsing it would
				     only squash it over the row moving in. -->
				<li
					class={list.row}
					in:fly={{ y: 8, duration: motion.duration }}
					out:fade={{ duration: motion.duration }}
					animate:flip={{ duration: motion.duration }}
				>
					<span class="flex self-center">
						<IconTile icon={account.connector.icon} title={account.connector.title} />
					</span>
					<div class={list.main}>
						<span>{account.description}</span>
						{#if account.host.kind === 'orphaned'}
							<span class="text-xs text-fg-subtle"
								>{account.connector.title}. No longer connected; move its resources to another
								account or remove it.</span
							>
						{:else if extension}<span class="text-xs text-fg-subtle">via {extension}</span>{/if}
						{#if account.credential_failed_at !== null}
							<span class="text-xs text-warn" role="status"
								>Sign-in failed. Sign in again to keep importing.</span
							>
						{/if}
					</div>
					<div class={list.actions}>
						{#if account.credential_failed_at !== null}
							<Button onclick={() => onreauthenticate(account)}>Sign in again</Button>
						{/if}
						<RowMenu label="More for {account.description}" items={actions(account)} />
					</div>
				</li>
			{:else}
				<li>
					<EmptyState
						compact
						message="No accounts yet"
						detail="Add one to import from a service."
					/>
				</li>
			{/each}
		</ul>
	{/snippet}
</ResourceState>

<MoveResourcesDialog source={moving} onclose={() => (moving = null)} />

<ConfirmDialog
	subject={removing}
	onclose={() => (removing = null)}
	title="Remove {removing?.description ?? 'account'}?"
	description="Everything imported with this account will be removed from every workspace."
	confirmLabel="Remove"
	onconfirm={(account) => connectors.remove(account.id)}
/>
