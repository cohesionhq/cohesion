export default function validate() {
  return B.check([[B.task('Hang the new shelves').assignee === null, 'unassigned'], [B.changedOnly(['Hang the new shelves']), `changed only Hang the new shelves (changed: ${B.changedTasks().join(', ')})`]])
}
