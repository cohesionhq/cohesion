import { redirect } from '@sveltejs/kit';
import { routes } from '$lib/shell/routes';

// `/connectors` opens Accounts.
export function load() {
	redirect(307, routes.connectorAccounts());
}
