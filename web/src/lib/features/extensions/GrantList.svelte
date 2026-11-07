<!--
	One installed extension's permissions or hooks, each with a switch, in the
	canonical order. A hook switch applies the next time the hook fires.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { vocabulary } from '$lib/protocol/vocabulary.svelte';
	import { hookTitle, extensions, sortHooks, sortPermissions } from '$lib/stores/extensions.svelte';
	import { list } from '$lib/ui/list';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';
	import GrantRow from './GrantRow.svelte';
	import { PERMISSION_ICONS } from './grants';

	interface Props {
		extensionId: number;
		kind: 'permissions' | 'hooks';
	}

	let { extensionId, kind }: Props = $props();

	const resource = $derived(
		kind === 'permissions' ? extensions.permissions(extensionId) : extensions.hooks(extensionId)
	);

	// Both kinds as one shape: how to show it, whether it's on, and how to turn it on or off.
	const rows = $derived(
		kind === 'permissions'
			? sortPermissions(extensions.permissions(extensionId).value, (p) => p.permission).map(
					(p) => ({
						name: p.permission,
						icon: PERMISSION_ICONS[p.permission],
						title: vocabulary.permission(p.permission).title,
						detail: vocabulary.permission(p.permission).description,
						enabled: p.enabled,
						set: (on: boolean) => extensions.setPermission(extensionId, p.permission, on)
					})
				)
			: sortHooks(extensions.hooks(extensionId).value, (h) => h.hook).map((h) => ({
					name: h.hook,
					icon: undefined,
					title: hookTitle(h.hook),
					detail: undefined,
					enabled: h.enabled,
					set: (on: boolean) => extensions.setHook(extensionId, h.hook, on)
				}))
	);

	let pending = $state<string | null>(null);

	$effect(() => {
		const current = resource;
		const id = extensionId;
		untrack(() => void current.load(id));
		return () => current.release();
	});

	async function toggle(row: (typeof rows)[number], enabled: boolean) {
		pending = row.name;
		try {
			await row.set(enabled);
		} catch (error) {
			toasts.error(error);
		} finally {
			pending = null;
		}
	}
</script>

{#if resource.error && !resource.loaded}
	<p class={list.error}>{resource.error.message}</p>
{:else if resource.loaded}
	{#if rows.length === 0}
		<p class={list.empty}>
			{kind === 'permissions'
				? 'This extension asks for no permissions.'
				: 'This extension runs on no hooks.'}
		</p>
	{:else}
		<ul class="flex flex-col divide-y divide-border rounded-md border border-border">
			{#each rows as row (row.name)}
				<GrantRow icon={row.icon} title={row.title} detail={row.detail} muted={!row.enabled}>
					{#snippet control()}
						<Switch
							checked={row.enabled}
							disabled={pending === row.name}
							label={row.title}
							onchange={(on) => void toggle(row, on)}
						/>
					{/snippet}
				</GrantRow>
			{/each}
		</ul>
	{/if}
{/if}
