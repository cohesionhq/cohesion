export default function validate() {
  const [task] = B.tasksLike(/visa/i);
  return B.check([
    [task?.dueDate === B.day(0), `due today (was ${task?.dueDate})`],
    [task?.priority === 'urgent', `urgent (was ${task?.priority})`],
  ])
}
