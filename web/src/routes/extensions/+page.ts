import { redirect } from '@sveltejs/kit';
import { routes } from '$lib/shell/routes';

// `/extensions` opens its Installed tab.
export function load() {
	redirect(307, routes.extensionsInstalled());
}
