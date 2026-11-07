<script lang="ts">
	import AboutSection from '$lib/features/settings/AboutSection.svelte';
	import DeveloperSection from '$lib/features/settings/DeveloperSection.svelte';
	import ClientsSection from '$lib/features/settings/ClientsSection.svelte';
	import IntelligenceSection from '$lib/features/settings/IntelligenceSection.svelte';
	import MemoriesSection from '$lib/features/settings/MemoriesSection.svelte';
	import ProfileSection from '$lib/features/settings/ProfileSection.svelte';
	import SecuritySection from '$lib/features/settings/SecuritySection.svelte';
	import StorageSection from '$lib/features/settings/StorageSection.svelte';
	import UsageSection from '$lib/features/settings/UsageSection.svelte';
	import WorkspacesSection from '$lib/features/settings/WorkspacesSection.svelte';
	import { SECTIONS } from '$lib/features/settings/sections';
	import PageTitle from '$lib/shell/PageTitle.svelte';

	let { data } = $props();

	const views = {
		profile: ProfileSection,
		workspaces: WorkspacesSection,
		intelligence: IntelligenceSection,
		memories: MemoriesSection,
		usage: UsageSection,
		storage: StorageSection,
		clients: ClientsSection,
		security: SecuritySection,
		developer: DeveloperSection,
		about: AboutSection
	};

	const View = $derived(views[data.section]);
	const label = $derived(SECTIONS.find((s) => s.slug === data.section)?.label ?? 'Settings');
</script>

<PageTitle title={`${label} · Settings`} />

{#key data.section}
	<View />
{/key}
