<!-- An agent's avatar: its emoji, or its initial when it has none, on a disc of its color. -->
<script lang="ts" module>
	import type { IntelligenceAgentColor } from '$lib/protocol/types';

	/** Who an avatar drawn elsewhere (a toast, a menu item) stands for. */
	export interface Avatar {
		name: string;
		color: IntelligenceAgentColor;
		emoji: string | null;
	}
</script>

<script lang="ts">
	import { agentFill } from './colors';

	interface Props {
		name: string;
		color: IntelligenceAgentColor;
		emoji?: string | null;
		/** `xs` for the sidebar's rows, `sm` for list rows, `md` for the conversation header, `lg` for cards. */
		size?: 'xs' | 'sm' | 'md' | 'lg';
		/** Draws the emoji at the sidebar's 0.45 of the disc, for toasts and the conversation header; `xs` always does. */
		tight?: boolean;
	}

	let { name, color, emoji = null, size = 'lg', tight = false }: Props = $props();

	const initial = $derived([...name.trim()][0]?.toUpperCase() ?? '?');

	const disc = { xs: 20, sm: 28, md: 32, lg: 44 };
	/** The glyph's font size as a share of the disc's diameter. */
	const glyph = { emoji: 0.6, initial: 0.5, tight: 0.45 };

	const share = $derived(
		!emoji ? glyph.initial : size === 'xs' || tight ? glyph.tight : glyph.emoji
	);
</script>

<!--
	The glyph is a share of the disc's diameter (`glyph`), the same on every
	client: 0.6 for an emoji, 0.5 for an initial. The sidebar's 20px disc and
	the toast and the conversation header take 0.45 instead, as 0.6 crowds them (`tight`). The disc
	clips its glyph, so an emoji that draws wider than its font size, or a
	value of several (the daemon allows up to 8 characters), stays inside the
	circle wherever the avatar appears, toasts included. See web/README.md.
-->
<span
	class={[
		'inline-flex size-(--disc) flex-none items-center justify-center overflow-hidden rounded-full leading-none font-semibold whitespace-nowrap text-on-fill select-none',
		agentFill[color]
	]}
	style:--disc="{disc[size]}px"
	style:font-size="{disc[size] * share}px"
	aria-hidden="true">{emoji ?? initial}</span
>
