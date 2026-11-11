export default function validate() {
  const [task] = B.tasksLike(/casserole/i);
  const priya = B.contact('Priya', 'Shah');
  return B.check([
    [task?.assignee?.kind === 'contact' && task.assignee.contactId === priya.id, 'assigned to Priya Shah'],
  ])
}
