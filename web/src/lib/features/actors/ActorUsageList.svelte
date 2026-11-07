<!--
	How much of its plan's limits each pool has used, as its actors' clients
	last said: one compact line an account several actors share, naming the
	account and where its actors work, or an actor whose limits are its own,
	naming where it works and its device, with a dial for each limit showing its
	percentage and when it resets. Only pools that report usage show; with none,
	nothing does. The label may truncate; hovering, focusing or tapping it opens the
	full list of the pool's actors.
-->
<script lang="ts">
	import type { Actor, ListedActorUsage } from '$lib/protocol/types';
	import { nowSeconds } from '$lib/format/dates';
	import { resetsLabel, shortLimitLabel } from './plan';
	import Popover from '$lib/ui/Popover.svelte';
	import UsageDial from './UsageDial.svelte';
	import { scopeName } from './words';

	let { actors, usage }: { actors: Actor[]; usage: ListedActorUsage[] } = $props();

	let openKey = $state<string | null>(null);

	const rows = $derived(
		usage.flatMap(({ actor_ids, pool, plan = [] }) => {
			const members = actors.filter((actor) => actor_ids.includes(actor.id));
			if (members.length === 0 || plan.length === 0) return [];
			const [first] = members;
			return pool
				? [
						{
							key: `pool:${pool.id}`,
							name: pool.name,
							detail: members.map(scopeName).join(', '),
							members,
							mono: false,
							plan
						}
					]
				: [
						{
							key: `actor:${first.id}`,
							name: scopeName(first),
							detail: first.title,
							members,
							mono: true,
							plan
						}
					];
		})
	);
</script>

{#if rows.length > 0}
	<section aria-labelledby="actor-usage-heading">
		<h2 id="actor-usage-heading" class="border-b border-border pb-2 text-md font-semibold">
			Usage
		</h2>
		<ul class="flex flex-col pt-1">
			{#each rows as { key, name, detail, mono, members, plan } (key)}
				<li class="flex min-h-9 flex-wrap items-center gap-x-5 gap-y-2 py-1.5">
					<div class="flex min-w-40 flex-1">
						<Popover bind:open={() => openKey === key, (value) => (openKey = value ? key : null)}>
							{#snippet trigger(props)}
								<button
									{...props}
									type="button"
									class="flex min-w-0 items-baseline gap-2 rounded-sm text-left"
									onpointerenter={(event) => {
										if (event.pointerType === 'mouse') openKey = key;
									}}
									onpointerleave={(event) => {
										if (event.pointerType === 'mouse' && openKey === key) openKey = null;
									}}
								>
									<span class={['text-sm', mono && 'font-mono']}>{name}</span>
									<span class="truncate text-xs text-fg-muted">{detail}</span>
								</button>
							{/snippet}
							<ul class="flex flex-col gap-1 px-2 py-1.5">
								{#each members as member (member.id)}
									<li class="flex items-baseline gap-2 text-xs">
										<span class="text-sm">{member.title}</span>
										<span class="font-mono text-fg-muted">{member.scope}</span>
									</li>
								{/each}
							</ul>
						</Popover>
					</div>
					{#each plan as limit, index (index)}
						<UsageDial
							share={limit.used}
							label={shortLimitLabel(limit.label)}
							resets={limit.resets_at == null ? null : resetsLabel(limit.resets_at, nowSeconds())}
						/>
					{/each}
				</li>
			{/each}
		</ul>
	</section>
{/if}
