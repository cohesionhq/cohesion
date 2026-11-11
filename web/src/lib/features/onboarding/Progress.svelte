<!--
	How close the setup is to working: a segment per step, filled once the
	step is done, the page being shown outlined, and the share done in words.
-->
<script lang="ts">
	import { STEPS, readiness, type Page, type Step } from './onboarding';

	interface Props {
		done: ReadonlySet<Step>;
		page: Page;
	}

	let { done, page }: Props = $props();

	const labels: Record<Step, string> = {
		workspace: 'Workspace',
		intelligence: 'Models',
		agent: 'Agent'
	};

	const share = $derived(readiness(done));
	const caption = $derived(share === 1 ? 'Ready to go' : `${Math.round(share * 100)}% ready`);
</script>

<div class="flex items-end gap-4">
	<ol class="flex flex-1 gap-1.5" aria-label="Setup progress">
		{#each STEPS as step (step)}
			<li class="flex flex-1 flex-col gap-1.5">
				<span
					class={[
						'h-1.5 rounded-full transition-colors',
						done.has(step) ? 'bg-accent' : 'bg-n2',
						page === step && 'ring-2 ring-accent/40'
					]}
					aria-hidden="true"
				></span>
				<span class={['text-2xs', page === step ? 'font-medium text-fg' : 'text-fg-subtle']}>
					{labels[step]}<span class="sr-only">{done.has(step) ? ', done' : ''}</span>
				</span>
			</li>
		{/each}
	</ol>
	<span class="pb-5 text-xs font-medium text-fg-muted tabular-nums">{caption}</span>
</div>
