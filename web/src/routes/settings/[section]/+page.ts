import { redirect } from '@sveltejs/kit';
import { isSection } from '$lib/features/settings/sections';
import { routes } from '$lib/shell/routes';

export function load({ params }) {
	// Extensions moved out of Settings; keep old links working.
	if (params.section === 'extensions') redirect(307, routes.extensionsInstalled());
	if (!isSection(params.section)) redirect(307, routes.settings('workspaces'));
	return { section: params.section };
}
