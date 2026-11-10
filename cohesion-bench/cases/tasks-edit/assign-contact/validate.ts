export default function validate() {
  const task = B.task('Fix the bike puncture');
  return B.check([
    [task.assignee?.kind === 'contact' && task.assignee.contactId === B.contact('Sam', 'Lee').id, 'assigned to Sam'],
    [B.changedOnly(['Fix the bike puncture']), `changed only Fix the bike puncture (changed: ${B.changedTasks().join(', ')})`],
  ])
}
