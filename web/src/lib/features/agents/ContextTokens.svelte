<!--
	An agent's context size, rolling as it changes. Room is held for "18k", so the
	roll from "—k" turns every digit rather than dropping a new one in beside it.
-->
<script lang="ts">
	import RollingText from '$lib/ui/RollingText.svelte';
	import { contextTokens, contextTokensLabel } from './contextTokens';

	let { tokens }: { tokens: number | null } = $props();
</script>

<RollingText text={contextTokens(tokens)} label={contextTokensLabel(tokens)} reserve={3} />
