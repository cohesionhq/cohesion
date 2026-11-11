export default function validate() {
  const [task] = B.tasksLike(/stamp/i, B.ANY);
  return B.check([
    [B.tasksLike(/stamp/i, B.ANY).length === 1, 'one task about stamps'],
    [task?.dueDate === B.day(3), `due ${B.day(3)} (was ${task?.dueDate})`],
    [task?.priority === 'high', `high priority (was ${task?.priority})`],
  ])
}
