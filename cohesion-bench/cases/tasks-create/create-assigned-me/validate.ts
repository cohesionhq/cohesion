export default function validate() {
  const [task] = B.tasksLike(/newsletter/i);
  return B.check([[task?.assignee?.kind === 'user', `assigned to the user (was ${JSON.stringify(task?.assignee)})`]])
}
