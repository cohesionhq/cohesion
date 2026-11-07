<!--
	One installed extension in the Installed list: its icon, name and version
	over its description, the whole row leading to its page. An Update button
	shows when a registry has a higher version, and an Inactive badge when
	none of its permissions and hooks are enabled yet (as after a fresh install).
-->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { untrack } from 'svelte';
	import type { ExtensionSummary, RegistryResult } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import { extensions } from '$lib/stores/extensions.svelte';
	import Button from '$lib/ui/Button.svelte';
	import IconTile from '$lib/ui/IconTile.svelte';
	import { grantNoun } from './grants';

	interface Props {
		extension: ExtensionSummary;
		update: RegistryResult | undefined;
		onupdate: (update: RegistryResult) => void;
	}

	let { extension, update, onupdate }: Props = $props();

	const permissions = $derived(extensions.permissions(extension.id));
	const hooks = $derived(extensions.hooks(extension.id));

	$effect(() => {
		const [p, h, id] = [permissions, hooks, extension.id];
		untrack(() => void Promise.all([p.load(id), h.load(id)]));
		return () => {
			p.release();
			h.release();
		};
	});

	// Which kinds to name while everything is off, else null.
	const inert = $derived.by(() => {
		if (!permissions.loaded || !hooks.loaded) return null;
		const grants = [...permissions.value, ...hooks.value];
		if (grants.length === 0 || grants.some((g) => g.enabled)) return null;
		return grantNoun(permissions.value.length, hooks.value.length);
	});
</script>

<li class="relative flex items-center gap-3 border-b border-border py-3">
	<IconTile icon={extension.icon} title={extension.name} size={32} />
	<div class="flex min-w-0 flex-1 flex-col">
		<span class="flex items-baseline gap-2">
			<!-- The link covers the row; the Update button sits above it. -->
			<a
				class="truncate text-fg after:absolute after:inset-0 after:rounded-sm hover:no-underline"
				href={routes.extension(extension.id)}>{extension.name}</a
			>
			<span class="flex-none text-xs text-fg-subtle tabular-nums">{extension.version}</span>
		</span>
		{#if extension.description}
			<span class="truncate text-xs text-fg-muted">{extension.description}</span>
		{/if}
	</div>
	{#if inert}
		<span
			class="inline-flex h-5 flex-none items-center rounded-sm bg-warn/12 px-1.5 text-2xs font-medium text-warn"
			>Inactive</span
		>
	{/if}
	{#if update}
		<Button
			class="relative border-border-strong!"
			aria-label="Update {extension.name} to {update.manifest.version}"
			onclick={() => onupdate(update)}
			>Update<span class="max-sm:hidden"> to {update.manifest.version}</span>…</Button
		>
	{/if}
	<ChevronRight class="flex-none text-fg-subtle" size={16} strokeWidth={1.75} aria-hidden="true" />
</li>
