<!-- Removes the articles feeds brought in before a day. -->
<script lang="ts">
	import { dayToDate, localDay } from '$lib/format/dates';
	import type { Feed } from '$lib/protocol/types';
	import { feeds as feedsStore } from '$lib/stores/feeds.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Checkbox from '$lib/ui/Checkbox.svelte';
	import Dialog from '$lib/ui/Dialog.svelte';
	import Field from '$lib/ui/Field.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		open?: boolean;
		feeds: Feed[];
		/** The feeds ticked when the dialog opens. */
		initial: number[];
	}

	let { open = $bindable(false), feeds, initial }: Props = $props();

	let day = $state(localDay(30));
	let chosen = $state<number[]>([]);
	let busy = $state(false);

	$effect(() => {
		if (open) chosen = [...initial];
	});

	function toggle(id: number, on: boolean) {
		chosen = on ? [...chosen, id] : chosen.filter((c) => c !== id);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		try {
			const before = Math.floor(dayToDate(day).getTime() / 1000);
			const { removed } = await feedsStore.prune(chosen, before);
			open = false;
			toasts.show(removed === 1 ? 'Removed 1 article.' : `Removed ${removed} articles.`);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open
	title="Clean up feeds"
	description="Cohesion will remove the articles published before the day you pick. This can’t be undone."
>
	<form id="clean-up-form" class="flex flex-col gap-4" onsubmit={submit}>
		<Field label="Published before" for="clean-up-day">
			<TextInput id="clean-up-day" type="date" bind:value={day} required />
		</Field>
		<fieldset class="flex flex-col gap-1.5">
			<legend class="mb-1.5 text-xs text-fg-muted">Feeds</legend>
			{#each feeds as feed (feed.id)}
				<Checkbox
					checked={chosen.includes(feed.id)}
					onchange={(e) => toggle(feed.id, e.currentTarget.checked)}>{feed.title}</Checkbox
				>
			{/each}
		</fieldset>
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button
			variant="danger-quiet"
			type="submit"
			form="clean-up-form"
			disabled={busy || chosen.length === 0 || day === ''}
		>
			{busy ? 'Removing…' : 'Remove'}
		</Button>
	{/snippet}
</Dialog>
