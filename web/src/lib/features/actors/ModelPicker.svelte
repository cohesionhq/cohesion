<!--
	Which of its actor's models a new session runs on and at what effort, as
	one menu bar chip naming both that opens onto a panel listing the
	models over an effort slider. A model without efforts hides the slider, whose
	first stop, "Default", leaves the effort to the model.
-->
<script lang="ts">
	import type { Actor } from '$lib/protocol/types';
	import Choices from '$lib/ui/Choices.svelte';
	import MenuBarItem from '$lib/ui/MenuBarItem.svelte';

	interface Props {
		models: Actor['models'];
		model: string;
		/** The effort's identifier, or '' for the model's default. */
		effort: string;
		/** Called once the user has picked either. */
		onpick?: () => void;
	}

	let { models, model = $bindable(), effort = $bindable(), onpick }: Props = $props();

	const efforts = $derived(models.find((m) => m.identifier === model)?.efforts ?? []);
	/** The slider's stops: the model's default first, then its efforts, least to most. */
	const stops = $derived([{ identifier: '', title: 'Default' }, ...efforts]);

	const label = $derived.by(() => {
		const parts = [models.find((m) => m.identifier === model)?.title ?? 'Choose a model'];
		const e = efforts.find((e) => e.identifier === effort);
		if (e) parts.push(e.title);
		return parts.join(' · ');
	});

	function pickModel(identifier: string) {
		model = identifier;
		const takes = models.find((m) => m.identifier === identifier)?.efforts ?? [];
		if (!takes.some((e) => e.identifier === effort)) effort = '';
		onpick?.();
	}

	function pickEffort(identifier: string) {
		effort = identifier;
		onpick?.();
	}

	const heading = 'px-2 pt-1.5 pb-1 text-xs text-fg-subtle';
</script>

<MenuBarItem {label} name="Model and effort">
	<div class="flex w-72 flex-col">
		<p class={heading}>Model</p>
		<Choices
			label="Model"
			choices={models.map((m) => ({ id: m.identifier, label: m.title }))}
			checked={model}
			onpick={(id) => pickModel(String(id))}
		/>

		{#if efforts.length > 0}
			<div class="-mx-1 my-1 h-px bg-border"></div>
			<p class={heading}>Effort</p>
			<!-- Least to most, left to right, as a scale reads; each stop's name sits under it and picks it too. -->
			<div class="flex flex-col gap-1 px-2 pb-1.5">
				<input
					class="w-full cursor-pointer accent-accent"
					type="range"
					min="0"
					max={stops.length - 1}
					step="1"
					value={stops.findIndex((e) => e.identifier === effort)}
					aria-label="Effort"
					aria-valuetext={stops.find((e) => e.identifier === effort)?.title}
					oninput={(event) => pickEffort(stops[event.currentTarget.valueAsNumber].identifier)}
				/>
				<div class="flex justify-between gap-1">
					{#each stops as e (e.identifier)}
						<button
							class={[
								'min-w-0 cursor-pointer truncate text-xs transition-colors first:text-left last:text-right',
								e.identifier === effort ? 'text-fg' : 'text-fg-subtle hover:text-fg'
							]}
							type="button"
							tabindex="-1"
							onclick={() => pickEffort(e.identifier)}>{e.title}</button
						>
					{/each}
				</div>
			</div>
		{/if}
	</div>
</MenuBarItem>
