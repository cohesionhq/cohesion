<!--
	A plan an actor asks the user to approve before it starts, inline in the
	transcript: the plan, then approving it or asking for changes, with what
	to change. Once decided, the card is read-only and the decision follows
	it.
-->
<script lang="ts">
	import Button from '$lib/ui/Button.svelte';
	import Markdown from '$lib/ui/Markdown.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		plan: string;
		answered: boolean;
		ondecide: (approved: boolean, feedback: string | null) => Promise<unknown>;
	}

	let { plan, answered, ondecide }: Props = $props();

	let feedback = $state('');
	let busy = $state(false);

	async function decide(approved: boolean) {
		busy = true;
		try {
			await ondecide(approved, feedback.trim() || null);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<article
	class="flex w-full max-w-[70%] min-w-0 flex-col gap-2 rounded-md border border-border bg-bg px-3 py-2.5 max-sm:max-w-full"
>
	<Markdown source={plan} diagrams />
	{#if !answered}
		<TextInput
			bind:value={feedback}
			placeholder="What to change (optional)"
			aria-label="What to change"
		/>
		<div class="flex flex-wrap justify-end gap-2">
			<Button disabled={busy} onclick={() => decide(false)}>Ask for changes</Button>
			<Button variant="primary" disabled={busy} onclick={() => decide(true)}>Approve plan</Button>
		</div>
	{/if}
</article>
