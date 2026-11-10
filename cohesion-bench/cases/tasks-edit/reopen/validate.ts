export default function validate() {
  return B.check([
    [B.task('Set up the ISA transfer').status === 'incomplete', 'open again'],
    [B.changedOnly(['Set up the ISA transfer']), `changed only Set up the ISA transfer (changed: ${B.changedTasks().join(', ')})`],
  ])
}
