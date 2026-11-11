export default function validate() {
  return B.check([
    [B.task('Read Piranesi').dueDate === null, 'no due date'],
    [B.changedOnly(['Read Piranesi']), `changed only Read Piranesi (changed: ${B.changedTasks().join(', ')})`],
  ])
}
