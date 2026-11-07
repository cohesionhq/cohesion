import { redirect } from '@sveltejs/kit';
import { routes } from '$lib/shell/routes';

// Find was renamed Browse; keep old links working.
export function load() {
	redirect(307, routes.extensionsBrowse());
}
