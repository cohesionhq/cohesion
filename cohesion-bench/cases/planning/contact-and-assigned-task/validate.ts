export default function validate() {
  const ana = B.findContact('Ana', 'Silva');
  const [task] = B.tasksLike(/chase ana/i);
  const launch = new Date(B.date(0).getFullYear(), 11, 1, 12);
  if (launch < B.date(0)) launch.setFullYear(launch.getFullYear() + 1);
  launch.setDate(launch.getDate() - 1);
  return B.check([
    [ana !== null, 'Ana Silva'],
    [task?.assignee?.kind === 'contact' && task.assignee.contactId === ana?.id, 'the task assigned to her'],
    [task?.dueDate === B.iso(launch), `due ${B.iso(launch)}, the day before 1 December (was ${task?.dueDate})`],
  ])
}
