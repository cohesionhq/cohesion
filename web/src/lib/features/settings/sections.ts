// The settings sections, in nav order. `slug` is the `/settings/[section]` segment.

export const SECTIONS = [
	{ slug: 'workspaces', label: 'Workspaces' },
	{ slug: 'profile', label: 'Profile' },
	{ slug: 'intelligence', label: 'Agents' },
	{ slug: 'memories', label: 'Memories' },
	{ slug: 'usage', label: 'Usage' },
	{ slug: 'storage', label: 'Storage' },
	{ slug: 'clients', label: 'Clients' },
	{ slug: 'security', label: 'Security' },
	{ slug: 'developer', label: 'Developer' },
	{ slug: 'about', label: 'About' }
] as const;

export type SectionSlug = (typeof SECTIONS)[number]['slug'];

export function isSection(slug: string): slug is SectionSlug {
	return SECTIONS.some((s) => s.slug === slug);
}
