<!--
	An installed extension's page: what it is, an Update button when a
	registry has a higher version, its permissions and hooks as switches, its
	identifier and run order, and uninstalling it. A fresh install has
	everything off, so the page says it's inactive until something is enabled.
-->
<script lang="ts">
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { untrack } from 'svelte';
	import Section from '$lib/features/settings/Section.svelte';
	import type { ExtensionSummary } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { extensions } from '$lib/stores/extensions.svelte';
	import PageFooter from '$lib/ui/PageFooter.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConfirmDialog from '$lib/ui/ConfirmDialog.svelte';
	import EmptyState from '$lib/ui/EmptyState.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { list } from '$lib/ui/list';
	import PageHeader from '$lib/ui/PageHeader.svelte';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import GrantList from './GrantList.svelte';
	import { grantNoun } from './grants';
	import ReviewExtensionDialog from './ReviewExtensionDialog.svelte';
	import { pane } from '$lib/shell/pane.svelte';
	import PageTitle from '$lib/shell/PageTitle.svelte';

	const page = pane();

	interface Props {
		id: number;
	}

	let { id }: Props = $props();

	let uninstalling = $state<ExtensionSummary | null>(null);
	let updateOpen = $state(false);

	$effect(() => {
		untrack(() => void extensions.load());
		return () => extensions.list.release();
	});

	const extension = $derived(extensions.list.value.find((e) => e.id === id));
	const update = $derived(extensions.updates.value.find((u) => u.extension_id === id)?.result);

	// The grant lists below load these; read here only to tell when all are
	// off, and which kinds to name when they are. Kept out of `inert` so it
	// tracks them: state a derived creates itself doesn't make it re-run.
	const permissions = $derived(extensions.permissions(id));
	const hooks = $derived(extensions.hooks(id));
	const inert = $derived.by(() => {
		if (!permissions.loaded || !hooks.loaded) return null;
		const grants = [...permissions.value, ...hooks.value];
		if (grants.length === 0 || grants.some((g) => g.enabled)) return null;
		return grantNoun(permissions.value.length, hooks.value.length);
	});

	async function uninstall(target: ExtensionSummary) {
		await extensions.uninstall(target.id);
		void page.goto(routes.extensionsInstalled(), { replaceState: true });
	}
</script>

<PageTitle title={`${extension ? `${extension.name} · Extensions` : 'Extensions'}`} />

<div class="px-(--page-inset)">
	<ResourceState resource={extensions.list}>
		{#snippet loaded()}
			{#if !extension}
				<PageHeader title="Extensions" />
				<EmptyState
					message="Extension not found"
					actionLabel="Back to installed extensions"
					onaction={() => void page.goto(routes.extensionsInstalled(), { replaceState: true })}
				/>
			{:else}
				<PageHeader
					title={extension.name}
					back={{ href: routes.extensionsInstalled(), label: 'Installed' }}
				>
					{#snippet leading()}
						<IconTile icon={extension.icon} title={extension.name} size={28} />
					{/snippet}
					{#snippet meta()}
						{[`Version ${extension.version}`, extension.author && `by ${extension.author}`]
							.filter(Boolean)
							.join(' · ')}
					{/snippet}
					{#snippet actions()}
						{#if update}
							<Button variant="primary" onclick={() => (updateOpen = true)}
								>Update to {update.manifest.version}…</Button
							>
						{/if}
					{/snippet}
				</PageHeader>

				{#if extension.description}
					<p class="mb-6 max-w-160 text-fg-muted">{extension.description}</p>
				{/if}

				{#if inert}
					<div
						class="mb-6 flex items-start gap-2 rounded-md bg-warn/12 px-3 py-2 text-warn [&_svg]:mt-0.5 [&_svg]:flex-none"
						role="status"
					>
						<TriangleAlert size={15} strokeWidth={1.75} aria-hidden="true" />
						<p>
							{extension.name} is inactive until you enable its {inert} below.
						</p>
					</div>
				{/if}

				<Section title="Permissions">
					<GrantList extensionId={extension.id} kind="permissions" />
				</Section>

				<Section title="Hooks" description="A change applies the next time it fires.">
					<GrantList extensionId={extension.id} kind="hooks" />
				</Section>

				<Section title="Details">
					<dl class={list.rows}>
						<div class={list.row}>
							<dt class={[list.main, 'text-fg-muted']}>Identifier</dt>
							<dd class="font-mono text-xs wrap-anywhere">{extension.script_identifier}</dd>
						</div>
						<div class={list.row}>
							<dt class={[list.main, 'text-fg-muted']}>Run order</dt>
							<dd class="text-xs">{extension.priority}</dd>
						</div>
					</dl>
				</Section>

				<PageFooter label="Extension">
					<Button
						class="-my-1 ml-auto h-6 px-1.5 text-xs"
						variant="danger-quiet"
						onclick={() => (uninstalling = extension)}>Uninstall…</Button
					>
				</PageFooter>
			{/if}
		{/snippet}
	</ResourceState>
</div>

<ReviewExtensionDialog bind:open={updateOpen} result={update ?? null} />
<ConfirmDialog
	subject={uninstalling}
	onclose={() => (uninstalling = null)}
	title="Uninstall {uninstalling?.name ?? 'extension'}?"
	description="This extension will stop running and lose the access you gave it."
	confirmLabel="Uninstall"
	onconfirm={uninstall}
/>
