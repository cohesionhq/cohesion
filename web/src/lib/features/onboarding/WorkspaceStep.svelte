<!--
	The flow's first page: names the first workspace and gives it an emoji,
	previewed large beside the title.
-->
<script lang="ts">
	import SmilePlus from '@lucide/svelte/icons/smile-plus';
	import { toProtocolError } from '$lib/protocol/errors';
	import type { Workspace } from '$lib/protocol/types';
	import { workspaces } from '$lib/stores/workspaces.svelte';
	import Button from '$lib/ui/Button.svelte';
	import EmojiPicker from '$lib/ui/EmojiPicker.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import StepPage from './StepPage.svelte';

	interface Props {
		/** The workspace made here, once it is. */
		created: Workspace | null;
		oncreated: (workspace: Workspace) => void;
		onnext: () => void;
	}

	let { created, oncreated, onnext }: Props = $props();

	const id = $props.id();
	let title = $state('');
	let emoji = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	const shown = $derived(created ? (created.emoji ?? '') : emoji);
	const initial = $derived([...(created?.title ?? title).trim()][0]?.toUpperCase() ?? '');

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = null;
		try {
			oncreated(await workspaces.create(title.trim(), emoji.trim() || null));
			onnext();
		} catch (e) {
			error = toProtocolError(e).message;
		} finally {
			busy = false;
		}
	}
</script>

<StepPage
	eyebrow="Welcome to Cohesion"
	title={created ? `${created.title} is ready.` : 'Every good idea needs a home.'}
	description={created
		? 'Your journal, documents and tasks will live here.'
		: 'A workspace keeps your journal, documents and tasks together. Give your first one a name and an emoji to match.'}
	onskip={created ? undefined : onnext}
>
	{#snippet art()}
		<span
			class="grid size-24 rotate-3 place-items-center rounded-md border border-border bg-bg-raised text-display leading-none shadow-(--shadow-floating)"
			aria-hidden="true"
		>
			{#if shown}
				{shown}
			{:else if initial}
				<span class="text-xl font-semibold text-fg-subtle">{initial}</span>
			{:else}
				<SmilePlus class="size-10 text-fg-subtle" strokeWidth={1.5} />
			{/if}
		</span>
	{/snippet}

	{#if !created}
		<form id="{id}-form" onsubmit={create}>
			<Field label="Name" for="{id}-title" {error}>
				<div class="flex gap-2">
					<EmojiPicker bind:value={emoji} />
					<TextInput
						id="{id}-title"
						bind:value={title}
						placeholder="Side projects"
						autocomplete="off"
						required
					/>
				</div>
			</Field>
		</form>
	{/if}

	{#snippet action()}
		{#if created}
			<Button variant="primary" onclick={onnext}>Continue</Button>
		{:else}
			<Button
				variant="primary"
				type="submit"
				form="{id}-form"
				disabled={busy || title.trim() === ''}
			>
				{busy ? 'Creating…' : 'Create workspace'}
			</Button>
		{/if}
	{/snippet}
</StepPage>
