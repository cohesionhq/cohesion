export default function validate() {
  const task = B.task('Write thank-you cards');
  return B.check([
    [task.dueDate === B.day(6), 'due date set'],
    [task.priority === 'high', 'high priority'],
    [B.changedOnly(['Write thank-you cards']), `changed only Write thank-you cards (changed: ${B.changedTasks().join(', ')})`],
  ])
}
