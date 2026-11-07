import { redirect } from '@sveltejs/kit';
import { routes } from '$lib/shell/routes';

// `/settings` opens the first section.
export function load() {
	redirect(307, routes.settings('workspaces'));
}
