<!--
	Settings › Storage: how large the database is, how long the hourly
	cleanup keeps agent messages, actor sessions, Rewind's versions and deleted
	items, and how many memories each scope keeps. Each number saves as
	it's entered. A backup writes a copy on the daemon's computer.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { fileSize } from '$lib/format/bytes';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { DatabaseBackup } from '$lib/protocol/types';
	import { storage } from '$lib/stores/settings/storage.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { list } from '$lib/ui/list';
	import ResourceState from '$lib/ui/ResourceState.svelte';
	import Section from './Section.svelte';
	import StorageNumberRow from './StorageNumberRow.svelte';

	$effect(() => {
		untrack(() => void storage.load());
		return () => storage.release();
	});

	const settings = $derived(storage.settings.value);
	const size = $derived(storage.size.value);

	let backingUp = $state(false);
	let backup = $state<DatabaseBackup | null>(null);
	let backupError = $state<string | null>(null);

	async function backUp() {
		backingUp = true;
		backupError = null;
		try {
			backup = await storage.backUp();
		} catch (error) {
			backupError = toProtocolError(error).message;
		} finally {
			backingUp = false;
		}
	}
</script>

<Section title="Database" description="Cohesion keeps everything in this file.">
	<ResourceState resource={storage.size}>
		{#snippet loaded()}
			{#if size}
				<ul class={list.rows}>
					<li class={list.row}>
						<div class={[list.main, 'flex-col']}>
							<span>Size</span>
							<span class="text-xs text-fg-subtle">
								{fileSize(size.free_bytes)} of this is free space. Cohesion reuses it before the file
								grows.
							</span>
						</div>
						<span class="ml-auto tabular-nums">{fileSize(size.bytes)}</span>
					</li>
					<li class={list.row}>
						<div class={[list.main, 'flex-col']}>
							<span>Backup</span>
							<span class="text-xs text-fg-subtle">
								Cohesion will write a copy to the computer it’s running on. The copy holds
								everything except images in actor sessions and agent messages.
							</span>
							{#if backup}
								<span class="text-xs break-all text-fg-subtle">
									{fileSize(backup.bytes)} at {backup.path}
								</span>
							{/if}
							{#if backupError}
								<p class="text-xs text-danger" role="alert">{backupError}</p>
							{/if}
						</div>
						<Button class="ml-auto" disabled={backingUp} onclick={backUp}>
							{backingUp ? 'Backing up…' : 'Back up'}
						</Button>
					</li>
				</ul>
			{/if}
		{/snippet}
	</ResourceState>
</Section>

<ResourceState resource={storage.settings}>
	{#snippet loaded()}
		{#if settings}
			<Section
				title="Retention"
				description="Cohesion clears these out every hour. Chats with agents are kept, apart from old script results."
			>
				<ul class={list.rows}>
					<StorageNumberRow
						label="Agent messages"
						help="Agents’ conversation messages as their models saw them: the model threads behind each chat, background work included. A thread is deleted once nothing is added to it for this long, and older messages in it and script results are emptied."
						unit="days"
						current={settings.thread_retention_days}
						onsave={(days) => storage.update({ thread_retention_days: days })}
					/>
					<StorageNumberRow
						label="Actor sessions"
						help="A session you’ve resolved is deleted with its images this long after you resolved it. Sessions you haven’t resolved are kept."
						unit="days"
						current={settings.actor_session_retention_days}
						onsave={(days) => storage.update({ actor_session_retention_days: days })}
					/>
					<StorageNumberRow
						label="Rewind versions"
						help="Earlier versions of tasks, documents and journal entries. A version is deleted this long after it was made, and a deleted item can’t come back once its versions are gone."
						unit="days"
						current={settings.rewind_retention_days}
						onsave={(days) => storage.update({ rewind_retention_days: days })}
					/>
					<StorageNumberRow
						label="Deleted items"
						help="A deleted task, document or journal entry is removed for good after this long, or once its Rewind versions have gone if that’s later."
						unit="days"
						current={settings.deleted_item_retention_days}
						onsave={(days) => storage.update({ deleted_item_retention_days: days })}
					/>
				</ul>
			</Section>

			<Section title="Memories" description="Agents remember these between conversations.">
				<ul class={list.rows}>
					<StorageNumberRow
						label="Memory capacity"
						help="Allow all agents, teams, and individual agents to have capacity for this many memories."
						unit="memories"
						current={settings.memory_limit}
						onsave={(limit) => storage.update({ memory_limit: limit })}
					/>
				</ul>
			</Section>
		{/if}
	{/snippet}
</ResourceState>
