// The settings sections, in nav order. `slug` is the `/settings/[section]` segment.

import Brain from '@lucide/svelte/icons/brain';
import ChartColumn from '@lucide/svelte/icons/chart-column';
import CircleUser from '@lucide/svelte/icons/circle-user';
import Database from '@lucide/svelte/icons/database';
import Info from '@lucide/svelte/icons/info';
import Layers from '@lucide/svelte/icons/layers';
import Monitor from '@lucide/svelte/icons/monitor-smartphone';
import Shield from '@lucide/svelte/icons/shield';
import Bot from '@lucide/svelte/icons/bot';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';

export const SECTIONS = [
	{ slug: 'workspaces', icon: Layers, label: 'Workspaces' },
	{ slug: 'profile', icon: CircleUser, label: 'Profile' },
	{ slug: 'intelligence', icon: Bot, label: 'Agents' },
	{ slug: 'memories', icon: Brain, label: 'Memories' },
	{ slug: 'usage', icon: ChartColumn, label: 'Usage' },
	{ slug: 'storage', icon: Database, label: 'Storage' },
	{ slug: 'clients', icon: Monitor, label: 'Clients' },
	{ slug: 'security', icon: Shield, label: 'Security' },
	{ slug: 'developer', icon: SquareTerminal, label: 'Developer' },
	{ slug: 'about', icon: Info, label: 'About' }
] as const;

export type SectionSlug = (typeof SECTIONS)[number]['slug'];

export function isSection(slug: string): slug is SectionSlug {
	return SECTIONS.some((s) => s.slug === slug);
}
