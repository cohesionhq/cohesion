import { authHeaders, markUnauthorized } from './session';
import type { ClientInfo } from './types';

/** What the CLI was opened with: its current workspace (if any) and daemon target. */
export async function getClientInfo(): Promise<ClientInfo> {
	const response = await fetch('/client', { headers: authHeaders() });
	if (response.status === 401) markUnauthorized();
	if (!response.ok) return { workspace: null, target: '' };
	return (await response.json()) as ClientInfo;
}
