<!-- The flow's last page: where each step stands, and the way into the workspace. -->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import type { Workspace } from '$lib/protocol/types';
	import Button from '$lib/ui/Button.svelte';
	import { STEPS, type Step } from './onboarding';
	import StepPage from './StepPage.svelte';

	interface Props {
		done: ReadonlySet<Step>;
		workspace: Workspace | null;
		onback: () => void;
		onfinish: () => void;
	}

	let { done, workspace, onback, onfinish }: Props = $props();

	const rows: Record<Step, { done: string; todo: string }> = {
		workspace: {
			done: 'A workspace to call home',
			todo: 'Create a workspace from this page whenever you’re ready.'
		},
		intelligence: {
			done: 'Models to think with',
			todo: 'Add a provider in Settings › Agents.'
		},
		agent: {
			done: 'An agent on your side',
			todo: 'Create an agent on the Agents page.'
		}
	};

	const all = $derived(STEPS.every((step) => done.has(step)));
</script>

<StepPage
	eyebrow="All done"
	title={all ? 'You’re all set.' : 'Good enough for now.'}
	description={all
		? 'Everything is in place. Time to make something.'
		: 'The rest can wait. Here’s what’s left whenever you want it.'}
	{onback}
>
	<ul class="flex flex-col gap-2">
		{#each STEPS as step (step)}
			{@const ok = done.has(step)}
			<li class="flex items-center gap-3">
				<span
					class={[
						'grid size-6 flex-none place-items-center rounded-full',
						ok ? 'bg-ok text-on-fill' : 'border border-dashed border-border-strong'
					]}
					aria-hidden="true"
				>
					{#if ok}<Check size={14} strokeWidth={3} />{/if}
				</span>
				<span class={ok ? 'font-medium' : 'text-fg-muted'}>
					{ok ? rows[step].done : rows[step].todo}
				</span>
			</li>
		{/each}
	</ul>

	{#snippet action()}
		<Button variant="primary" onclick={onfinish}>
			{workspace ? `Open ${workspace.title}` : 'Done'}
		</Button>
	{/snippet}
</StepPage>
