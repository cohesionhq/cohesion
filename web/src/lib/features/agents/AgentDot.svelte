<!-- An agent's color as a small dot. -->
<script lang="ts">
	import type { IntelligenceAgentColor } from '$lib/protocol/types';
	import { agentFill } from '$lib/ui/colors';

	interface Props {
		color: IntelligenceAgentColor;
		/** `lg` for the conversation header. */
		size?: 'sm' | 'lg';
	}

	let { color, size = 'sm' }: Props = $props();
</script>

<span
	class={[
		'inline-block flex-none rounded-full',
		agentFill[color],
		size === 'lg' ? 'size-3' : 'size-2'
	]}
	aria-hidden="true"
></span>
