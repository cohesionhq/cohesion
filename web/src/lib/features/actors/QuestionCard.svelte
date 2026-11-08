<!--
	An actor's questions, inline in the transcript, as one card: each
	question under its header, with its options to pick (one, or several
	where it allows) and, where it allows, an answer in the user's own
	words. Sending answers every question at once. Once answered, the card
	is read-only and holds the response itself: the options picked lit and
	checked, the rest faded, and any answer in words as a picked row of its
	own. Picks and words not yet sent are kept in this browser under
	`draftKey`, so leaving the session or reloading doesn't lose them.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import Check from '@lucide/svelte/icons/check';
	import type { ActorAnswer, ActorQuestion } from '$lib/protocol/types';
	import { rememberDraft, rememberedDraft } from '$lib/stores/drafts';
	import Button from '$lib/ui/Button.svelte';
	import TextInput from '$lib/ui/TextInput.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		/** Where this browser keeps the answers picked so far. */
		draftKey: string;
		questions: ActorQuestion[];
		answered: boolean;
		/** The user's response, one answer per question, once they gave one. */
		answers: ActorAnswer[] | null;
		onanswer: (answers: ActorAnswer[]) => Promise<unknown>;
	}

	let { draftKey, questions, answered, answers, onanswer }: Props = $props();

	interface Picks {
		picked: string[][];
		other: string[];
	}

	const id = $props.id();
	// A request's questions never change, so the picks start from them (or
	// what was kept of them) once.
	const kept = untrack(() => keptPicks());
	let picked = $state<string[][]>(kept.picked);
	let other = $state<string[]>(kept.other);
	let busy = $state(false);

	/** The picks kept for these questions, where they still fit them; none at first. */
	function keptPicks(): Picks {
		const stored = answered ? null : rememberedDraft<Partial<Picks> | null>(draftKey, null);
		return {
			picked: questions.map((question, i) => {
				const labels = stored?.picked?.[i];
				return Array.isArray(labels)
					? labels.filter((label) => question.options.some((o) => o.label === label))
					: [];
			}),
			other: questions.map((_, i) => {
				const words = stored?.other?.[i];
				return typeof words === 'string' ? words : '';
			})
		};
	}

	// Once answered, from here or anywhere else, there is nothing left to keep.
	$effect(() => {
		const blank = picked.every((p) => p.length === 0) && other.every((o) => o.trim() === '');
		rememberDraft(draftKey, answered || blank ? null : { picked, other });
	});

	/** Whether the question at `index` takes an answer in words. */
	const takesWords = (question: ActorQuestion) =>
		question.allows_other || question.options.length === 0;

	/** What is picked for the question at `index`: the response once there is one, else the user's picks so far. */
	const chosen = (index: number) => (answered ? (answers?.[index]?.selected ?? []) : picked[index]);

	function toggle(index: number, label: string) {
		const question = questions[index];
		const current = picked[index] ?? [];
		picked[index] = current.includes(label)
			? current.filter((l) => l !== label)
			: question.multi_select
				? [...current, label]
				: [label];
	}

	const complete = $derived(
		questions.every((_, i) => (picked[i]?.length ?? 0) > 0 || (other[i] ?? '').trim() !== '')
	);

	async function send(event: SubmitEvent) {
		event.preventDefault();
		if (!complete || busy) return;
		busy = true;
		try {
			await onanswer(
				questions.map((_, i) => ({
					selected: picked[i] ?? [],
					other: (other[i] ?? '').trim() || null
				}))
			);
		} catch (error) {
			toasts.error(error);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet mark(on: boolean, multi: boolean)}
	<span
		class={[
			'mt-0.5 grid size-4 flex-none place-items-center border transition-colors',
			multi ? 'rounded-sm' : 'rounded-full',
			on ? 'border-accent bg-accent text-accent-fg' : 'border-border-strong dark:border-fg-subtle'
		]}
	>
		{#if on}<Check size={10} strokeWidth={3} aria-hidden="true" />{/if}
	</span>
{/snippet}

<form
	class="flex w-full min-w-0 flex-col overflow-hidden rounded-md border border-border bg-bg"
	onsubmit={send}
>
	{#each questions as question, index (index)}
		{@const said = answered ? (answers?.[index]?.other ?? null) : null}
		<!-- The padding sits on a wrapper: a fieldset draws its legend over its own padding. -->
		<div class="px-3 py-2.5 [&+&]:border-t [&+&]:border-border">
			<fieldset class="flex min-w-0 flex-col gap-0.5" disabled={answered}>
				<legend class="mb-1.5 flex flex-col items-start">
					{#if question.header}
						<span class="text-xs font-medium text-fg-subtle">{question.header}</span>
					{/if}
					<span class="wrap-anywhere">{question.question}</span>
				</legend>
				{#each question.options as option (option.label)}
					{@const on = chosen(index)?.includes(option.label) ?? false}
					<button
						class={[
							'flex w-full items-start gap-2.5 rounded-sm px-2 py-1.25 text-left transition-colors',
							on
								? 'bg-accent-soft text-fg shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--accent)_40%,transparent)]'
								: answered
									? 'text-fg-subtle'
									: 'cursor-pointer hover:bg-bg-hover'
						]}
						type="button"
						role={question.multi_select ? 'checkbox' : 'radio'}
						aria-checked={on}
						onclick={() => toggle(index, option.label)}
					>
						{@render mark(on, question.multi_select)}
						<span class="flex min-w-0 flex-1 flex-col">
							<span class="wrap-anywhere">{option.label}</span>
							{#if option.description}
								<span class={['text-xs wrap-anywhere', on ? 'text-fg-muted' : 'text-fg-subtle']}
									>{option.description}</span
								>
							{/if}
						</span>
					</button>
				{/each}
				{#if said}
					<p class="flex items-start gap-2.5 rounded-sm bg-accent-soft px-2 py-1.25">
						{@render mark(true, question.multi_select)}
						<span class="min-w-0 flex-1 wrap-anywhere whitespace-pre-wrap">{said}</span>
					</p>
				{:else if takesWords(question) && !answered}
					<!-- Under the options, at their text's left edge (past the row's inset and its mark), so
					     the card's answers share one edge; alone, with no options, it takes the whole width. -->
					<div class={['mt-1', question.options.length > 0 && 'pl-8.5']}>
						<TextInput
							id="{id}-other-{index}"
							bind:value={other[index]}
							placeholder={question.options.length ? 'Something else…' : 'Your answer…'}
							aria-label="Answer in your own words"
						/>
					</div>
				{/if}
			</fieldset>
		</div>
	{/each}
	{#if !answered}
		<div class="flex justify-end px-3 pb-2.5">
			<Button variant="primary" type="submit" disabled={!complete || busy}
				>{busy ? 'Sending…' : 'Send answers'}</Button
			>
		</div>
	{/if}
</form>
