export default function validate() {
  return B.check([
    [!B.tagged(B.task('Hear back from the landlord'), 'waiting'), 'no longer waiting'],
    [B.changedOnly(['Hear back from the landlord']), `changed only Hear back from the landlord (changed: ${B.changedTasks().join(', ')})`],
  ])
}
