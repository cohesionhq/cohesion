// What ⌘T (where the Tasks page has no composer) and the palette's "New task" ask for: the new-task dialog.
// `NewTaskDialog` (in the root layout) answers, so any page can.

class NewTaskRequests {
	open = $state(false);

	request() {
		this.open = true;
	}
}

export const newTaskDialog = new NewTaskRequests();
