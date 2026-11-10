const w = cohesion.workspace('Personal');
const errands = w.taskFolders().find((f) => f.title === 'Errands');
for (const task of w.tasks()) {
  if (['Buy milk', 'Buy eggs and bread', 'Get coffee beans'].includes(task.title)) task.moveTo(errands);
}
