export default function validate() {
  const [task] = B.tasksLike(/window clean/i);
  return B.check([
    [B.tasksLike(/window clean/i).length === 1, 'one window cleaner task'],
    [task?.dueDate === B.day(1), `due tomorrow, ${B.day(1)} (was ${task?.dueDate})`],
  ])
}
