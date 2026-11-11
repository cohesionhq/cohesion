export default function validate() {
  const task = B.task('Book the dentist');
  return B.check([
    [task.dueDate === B.day(12), `due ${B.day(12)}`],
    [task.description.trim().length > 0, 'a note on why'],
  ])
}
