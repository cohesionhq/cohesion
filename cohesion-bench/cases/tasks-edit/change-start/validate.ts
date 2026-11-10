export default function validate() {
  const task = B.task('Repaint the fence');
  return B.check([
    [task.startDate === B.day(5), 'starts on the new day'],
    [task.dueDate === B.day(30), 'due date kept'],
    [B.changedOnly(['Repaint the fence']), `changed only Repaint the fence (changed: ${B.changedTasks().join(', ')})`],
  ])
}
