export default function validate() {
  const [task] = B.tasksLike(/newsletter/i, B.ANY);
  return B.check([[task?.assignee?.kind === 'user', `assigned to the user (was ${JSON.stringify(task?.assignee)})`]])
}
