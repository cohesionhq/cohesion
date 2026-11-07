<!--
	Adds an account through an extension granted `oauth`: it names a sign-in
	page, which opens in a new tab; the code it shows at the end is pasted
	back here and handed to the extension. Given an `account`, it signs that
	account in again: the extension replaces its credential when the sign-in is
	for the same remote account.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { ConnectorService, RedactedConnectorAccount } from '$lib/protocol/types';
	import { connectors } from '$lib/stores/connectors.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		service: ConnectorService | null;
		/** The account being signed in again, if any. */
		account?: RedactedConnectorAccount | null;
	}

	let { open = $bindable(false), service, account = null }: Props = $props();

	/** The sign-in page, fetched when the dialog opens so the click can open it at once. */
	let url = $state<string | null>(null);
	let urlError = $state<string | null>(null);
	let code = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);
	let generation = 0;

	$effect(() => {
		if (!open || !service) return;
		const id = service.extension_id;
		untrack(() => void begin(id));
		return () => generation++;
	});

	function reset() {
		url = null;
		urlError = null;
		code = '';
		error = null;
	}

	async function begin(extension_id: number) {
		const current = ++generation;
		try {
			const found = await connectors.beginAuthentication(extension_id);
			if (current === generation) url = found.url;
		} catch (e) {
			if (current === generation) urlError = toProtocolError(e).message;
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!service) return;
		busy = true;
		error = null;
		try {
			const signedIn = await connectors.completeAuthentication(service.extension_id, code.trim());
			if (account && signedIn.id !== account.id)
				toasts.show(
					`Signed in as ${signedIn.description}, a different account. Cohesion added that account.`
				);
			open = false;
		} catch (e) {
			const failure = toProtocolError(e);
			if (failure.code === 'validationFailed' || failure.code === 'upstreamError')
				error = failure.message;
			else toasts.error(failure);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet hero()}
	{#if service}<IconTile icon={service.icon} title={service.title} size={48} />{/if}
{/snippet}

<Dialog
	bind:open
	onclosed={reset}
	centered
	{hero}
	title={account
		? `Sign in again to ${account.description}`
		: `Add a ${service?.title ?? 'service'} account`}
	description={service?.sign_in_hint ??
		'Sign in on the page that opens, then paste the code it shows here.'}
>
	<form id="add-account-form" class="flex flex-col gap-3" onsubmit={submit}>
		{#if urlError !== null}
			<p class="text-xs whitespace-pre-wrap text-danger" role="alert">{urlError}</p>
		{:else}
			<Button
				class="self-start border-border-strong!"
				disabled={url === null}
				onclick={() => url && window.open(url, '_blank', 'noopener')}
			>
				{url === null ? 'Preparing sign-in…' : 'Open sign-in page'}
			</Button>
		{/if}
		<Field label="Code" for="account-code" {error}>
			<TextInput id="account-code" bind:value={code} autocomplete="off" spellcheck="false" />
		</Field>
	</form>
	{#snippet footer()}
		<Button
			variant="primary"
			type="submit"
			form="add-account-form"
			disabled={busy || code.trim() === ''}
		>
			{account ? (busy ? 'Signing in…' : 'Sign in') : busy ? 'Adding…' : 'Add account'}
		</Button>
		<Button class="border-border-strong!" onclick={() => (open = false)}>Cancel</Button>
	{/snippet}
</Dialog>
