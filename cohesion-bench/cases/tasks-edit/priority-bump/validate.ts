export default function validate() {
  return B.check([
    [B.task('Book the dentist').priority === 'urgent', 'dentist urgent'],
    [B.changedOnly(['Book the dentist']), `changed only Book the dentist (changed: ${B.changedTasks().join(', ')})`],
  ])
}
