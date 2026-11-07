<script lang="ts">
	import type { IntelligenceAgent } from '$lib/protocol/types';
	import { routes } from '$lib/shell/routes';
	import AgentAvatar from '$lib/ui/AgentAvatar.svelte';
	import Shimmer from '$lib/ui/Shimmer.svelte';

	interface Props {
		agents: IntelligenceAgent[];
		selected: number | null;
		onnavigate?: () => void;
	}

	let { agents, selected, onnavigate }: Props = $props();
</script>

<ul class="flex flex-col gap-px">
	{#each agents as agent (agent.id)}
		<li>
			<a
				href={routes.agent(agent.id)}
				class={[
					'flex flex-col rounded-sm px-2 py-1.5 text-fg transition-colors hover:bg-bg-hover hover:no-underline',
					agent.id === selected && 'bg-bg-hover'
				]}
				aria-current={agent.id === selected ? 'page' : undefined}
				onclick={onnavigate}
			>
				<span class="flex items-center gap-2.5">
					<span class="relative flex flex-none">
						<AgentAvatar
							name={agent.agent_name}
							color={agent.agent_color}
							emoji={agent.agent_emoji}
							size="sm"
						/>
						<!-- The accent while the agent has posted something unread; ringed in the row's
						     own background, so it cuts out of the avatar. -->
						{#if agent.agent_unread}
							<span
								class={[
									'absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full bg-accent ring-2 transition-colors',
									agent.id === selected ? 'ring-bg-hover' : 'ring-bg'
								]}
								aria-hidden="true"
							></span>
						{/if}
					</span>
					<span class="flex min-w-0 flex-1 flex-col">
						<Shimmer
							class={['truncate', (agent.id === selected || agent.agent_unread) && 'font-medium']}
							active={agent.activity !== null}>{agent.agent_name}</Shimmer
						>
						<span
							class={['truncate text-xs', agent.agent_unread ? 'text-fg-muted' : 'text-fg-subtle']}
							>{agent.agent_role}</span
						>
					</span>
					{#if agent.activity !== null}<span class="sr-only">, working</span>{/if}
					{#if agent.agent_unread}
						<span class="sr-only">Unread messages</span>
					{/if}
				</span>
			</a>
		</li>
	{/each}
</ul>
