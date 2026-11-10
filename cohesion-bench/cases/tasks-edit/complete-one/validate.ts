export default function validate() {
  return B.check([
    [B.task('Buy printer ink').status === 'completed', 'printer ink done'],
    [B.changedOnly(['Buy printer ink']), `changed only Buy printer ink (changed: ${B.changedTasks().join(', ')})`],
  ])
}
