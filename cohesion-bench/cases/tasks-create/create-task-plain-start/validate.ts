export default function validate() {
  const tasks = B.tasksLike(/stamp/i);
  const task = tasks[0];
  return B.check([
    [tasks.length === 1, `one task about stamps (found ${tasks.length})`],
    [task?.dueDate === '2026-11-14', 'due 2026-11-14'],
    [task?.priority === 'high', 'high priority'],
  ])
}
