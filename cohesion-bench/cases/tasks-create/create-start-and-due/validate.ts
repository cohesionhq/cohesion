export default function validate() {
  const [task] = B.tasksLike(/quarterly report/i, B.ANY);
  return B.check([
    [task?.startDate === B.day(7), `starts ${B.day(7)} (was ${task?.startDate})`],
    [task?.dueDate === B.day(14), `due ${B.day(14)} (was ${task?.dueDate})`],
  ])
}
