export default function validate() {
  return B.check([
    [B.task('Cancel the old phone contract').dueDate === B.day(0), 'the high-priority phone contract moved to today'],
    [B.task('Cancel the old phone contract').status === 'incomplete', 'the phone contract still open'],
    [B.task('Return library books').status === 'cancelled', 'library books cancelled'],
    [B.task('Renew the gym membership').status === 'cancelled', 'gym membership cancelled'],
    [B.changedOnly(['Cancel the old phone contract', 'Return library books', 'Renew the gym membership']), `changed only Cancel the old phone contract, Return library books, Renew the gym membership (changed: ${B.changedTasks().join(', ')})`],
  ])
}
