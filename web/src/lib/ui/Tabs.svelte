<!--
	A row of links between a page's views, drawn as underlined tabs. Sits on
	a rule (PageHeader's), which the current tab's underline covers.
-->
<script lang="ts">
	interface Tab {
		href: string;
		label: string;
		current: boolean;
		/** A small count after the label, e.g. updates waiting. */
		badge?: number;
	}

	interface Props {
		/** The landmark's name. */
		label: string;
		tabs: Tab[];
	}

	let { label, tabs }: Props = $props();
</script>

<nav
	class="-mb-px flex [scrollbar-width:none] gap-5 overflow-x-auto overscroll-y-auto"
	aria-label={label}
>
	{#each tabs as tab (tab.href)}
		<a
			class={[
				'inline-flex items-center gap-1.5 border-b-2 pb-2.5 whitespace-nowrap hover:no-underline',
				tab.current
					? 'border-fg font-medium text-fg'
					: 'border-transparent text-fg-muted hover:text-fg'
			]}
			href={tab.href}
			aria-current={tab.current ? 'page' : undefined}
		>
			{tab.label}
			{#if tab.badge}
				<span
					class="inline-flex h-4.5 min-w-4.5 items-center justify-center rounded-full bg-accent px-1 text-2xs font-semibold text-accent-fg tabular-nums"
					>{tab.badge}</span
				>
			{/if}
		</a>
	{/each}
</nav>
