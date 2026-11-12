// Whether the sidebar is collapsed to a restore button, remembered on this device.

const STORAGE_KEY = 'cohesion.sidebar-collapsed';

function readStored(): boolean {
	try {
		return localStorage.getItem(STORAGE_KEY) === '1';
	} catch {
		return false;
	}
}

function writeStored(collapsed: boolean): void {
	try {
		localStorage.setItem(STORAGE_KEY, collapsed ? '1' : '0');
	} catch {
		// Remembering the state is a convenience; ignore blocked storage.
	}
}

class SidebarState {
	collapsed = $state(false);

	/** Reads the remembered state; called once the page is in a browser. */
	restore(): void {
		this.collapsed = readStored();
	}

	set(collapsed: boolean): void {
		this.collapsed = collapsed;
		writeStored(collapsed);
	}

	toggle(): void {
		this.set(!this.collapsed);
	}
}

export const sidebarState = new SidebarState();
