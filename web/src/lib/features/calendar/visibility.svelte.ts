// Which calendars this browser hides from the calendar page. A view choice of
// this client's, kept in localStorage like the sidebar's width; a hidden
// calendar still syncs and its events are still found by search.

const KEY = 'cohesion.hiddenCalendars';

function remembered(): number[] {
	try {
		const stored = JSON.parse(localStorage.getItem(KEY) ?? '[]');
		return Array.isArray(stored) ? stored.filter((id) => typeof id === 'number') : [];
	} catch {
		return [];
	}
}

class CalendarVisibility {
	hidden = $state<number[]>(remembered());

	isShown(calendar_id: number): boolean {
		return !this.hidden.includes(calendar_id);
	}

	setShown(calendar_id: number, shown: boolean): void {
		this.hidden = shown
			? this.hidden.filter((id) => id !== calendar_id)
			: [...this.hidden.filter((id) => id !== calendar_id), calendar_id];
		try {
			localStorage.setItem(KEY, JSON.stringify(this.hidden));
		} catch {
			// Private browsing: the choice lasts until the page reloads.
		}
	}
}

export const calendarVisibility = new CalendarVisibility();
