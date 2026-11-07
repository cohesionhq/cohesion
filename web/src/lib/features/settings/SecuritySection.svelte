<!--
	Settings › Security: who can add clients, alerts, revoking clients, and
	the security activity. Each setting applies as it's picked.
-->
<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { ago } from '$lib/format/dates';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import type { PairingOpener, UpdateSecuritySettingsRequest } from '$lib/protocol/types';
	import { clients } from '$lib/stores/settings/clients.svelte';
	import { security } from '$lib/stores/settings/security.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import { list } from '$lib/ui/list';
	import Pager, { rememberedPageSize } from '$lib/ui/Pager.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Select from '$lib/ui/Select.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import Section from './Section.svelte';

	let revokingOthers = $state(false);
	let busy = $state(false);

	$effect(() => {
		untrack(() => {
			void security.load(rememberedPageSize());
			void clients.load();
		});
		return () => {
			security.release();
			clients.release();
		};
	});

	const settings = $derived(security.settings.value);
	const openerOptions = $derived(
		vocabulary.value.pairing_openers.map(({ kind, label }) => ({ value: kind, label }))
	);
	const inactiveOptions = [
		{ value: '', label: 'Never' },
		{ value: '30', label: 'After 30 days' },
		{ value: '90', label: 'After 90 days' },
		{ value: '180', label: 'After 180 days' },
		{ value: '365', label: 'After a year' }
	];

	async function change(update: Partial<UpdateSecuritySettingsRequest>) {
		busy = true;
		try {
			await security.update(update);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}

	async function revokeOthers() {
		const count = await clients.revokeOthers();
		toasts.show(count === 1 ? 'Revoked 1 client.' : `Revoked ${count} clients.`);
	}
</script>

{#snippet setting(label: string, help: string, control: Snippet)}
	<li class={list.row}>
		<div class={[list.main, 'flex-col']}>
			<span>{label}</span>
			<span class="text-xs text-fg-subtle">{help}</span>
		</div>
		<div class={[list.actions, 'w-56 justify-end max-sm:w-full']}>{@render control()}</div>
	</li>
{/snippet}

<ResourceState resource={security.settings}>
	{#snippet loaded()}
		{#if settings}
			<Section title="Pairing" description="Choose how new clients are added.">
				<ul class={list.rows}>
					{#snippet openedBy()}
						<Select
							class="w-full"
							value={settings.pairing_opened_by}
							options={openerOptions}
							aria-label="Add clients from"
							disabled={busy}
							onchange={(value) => void change({ pairing_opened_by: value as PairingOpener })}
						/>
					{/snippet}
					{@render setting(
						'Add clients from',
						'Choose which clients can show a setup code with Add client.',
						openedBy
					)}
				</ul>
			</Section>

			<Section
				title="Alerts"
				description="Cohesion sends notifications to your open apps and to extensions that deliver them."
			>
				<ul class={list.rows}>
					{#snippet newClient()}
						<Switch
							checked={settings.alert_new_client}
							label="New client paired"
							disabled={busy}
							onchange={(checked) => void change({ alert_new_client: checked })}
						/>
					{/snippet}
					{@render setting(
						'New client paired',
						'Cohesion sends an alert when a client is added.',
						newClient
					)}
				</ul>
			</Section>

			<Section title="Clients" description="Cohesion can revoke clients you no longer use.">
				<ul class={list.rows}>
					{#snippet inactive()}
						<Select
							class="w-full"
							value={settings.revoke_inactive_after_days?.toString() ?? ''}
							options={inactiveOptions}
							aria-label="Revoke clients that stop connecting"
							disabled={busy}
							onchange={(value) =>
								void change({ revoke_inactive_after_days: value === '' ? null : Number(value) })}
						/>
					{/snippet}
					{@render setting(
						'Revoke clients that stop connecting',
						'A client that’s connected now is never revoked.',
						inactive
					)}
					{#snippet others()}
						<Button variant="danger-quiet" onclick={() => (revokingOthers = true)}>
							Revoke others…
						</Button>
					{/snippet}
					{@render setting(
						'Revoke every other client',
						'Use this for a lost or stolen device. Every other client has to be set up again.',
						others
					)}
				</ul>
			</Section>
		{/if}
	{/snippet}
</ResourceState>

<Section
	title="Activity"
	description="This shows pairing, revoked clients and changes to these settings."
>
	<ResourceState resource={security.activity}>
		{#snippet loaded()}
			<ul class={list.rows}>
				{#each security.activity.value.events as event (event.id)}
					<li class={list.row}>
						<div class={list.main}>
							<span>{vocabulary.securityEvent(event.kind)}</span>
							{#if event.client_name}<span class="text-fg-muted">{event.client_name}</span>{/if}
							{#if event.address}
								<span class="font-mono text-xs text-fg-subtle">{event.address}</span>
							{/if}
						</div>
						<span class="text-xs text-fg-subtle tabular-nums">{ago(event.created_at)}</span>
					</li>
				{:else}
					<li class={list.empty}>Nothing yet.</li>
				{/each}
			</ul>
			<Pager
				offset={security.activity.key ?? 0}
				limit={security.pageSize}
				total={security.activity.value.total}
				onpage={(offset) => void security.page(offset)}
				onsize={(limit) => void security.resize(limit)}
			/>
		{/snippet}
	</ResourceState>
</Section>

<ConfirmDialog
	bind:open={revokingOthers}
	title="Revoke every other client?"
	description="Every other client won’t be able to connect or offer anything to Cohesion until it’s set up again. Any actors they offer will also be removed, but their tasks won’t be deleted."
	confirmLabel="Revoke"
	onconfirm={revokeOthers}
/>
