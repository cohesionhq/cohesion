<!--
	Settings › Workspaces › Modules: a switch per module saying whether it is
	on in one workspace. A module turned off leaves the sidebar, ⌘K and its
	page there; its data stays. A switch applies as it's flipped.
-->
<script lang="ts">
	import type { Module, Workspace } from '$lib/protocol/types';
	import { modules } from '$lib/stores/modules.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Switch from '$lib/ui/Switch.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		/** The workspace whose modules show; `null` closes the dialog. */
		workspace: Workspace | null;
		onclose: () => void;
	}

	let { workspace, onclose }: Props = $props();

	let busy = $state(false);

	$effect(() => {
		if (workspace === null) return;
		void modules.edited.load(workspace.id);
		return () => modules.edited.release();
	});

	async function flip(workspace_id: number, module: Module, enabled: boolean) {
		busy = true;
		try {
			await modules.set(workspace_id, module, enabled);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open={() => workspace !== null, (open) => !open && onclose()}
	title="Modules in “{workspace?.title ?? ''}”"
	description="Turn off a module to hide it in this workspace. The module’s data won’t be deleted."
>
	<ResourceState resource={modules.edited}>
		{#snippet loaded()}
			<ul class={list.rows}>
				{#each modules.edited.value as setting (setting.module)}
					<li class={list.row}>
						<div class={[list.main, 'flex-col']}>
							<span>{setting.title}</span>
							<span class="text-xs text-fg-subtle">{setting.description}</span>
						</div>
						<div class={[list.actions, 'justify-end']}>
							<Switch
								checked={setting.enabled}
								label={setting.title}
								disabled={busy || workspace === null}
								onchange={(checked) =>
									workspace && void flip(workspace.id, setting.module, checked)}
							/>
						</div>
					</li>
				{/each}
			</ul>
		{/snippet}
	</ResourceState>
	{#snippet footer()}
		<Button class="border-border-strong!" onclick={onclose}>Done</Button>
	{/snippet}
</Dialog>
