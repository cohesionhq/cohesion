export default function validate() {
  const overdue = ['Return library books', 'Renew the gym membership', 'Cancel the old phone contract'];
  return B.check([
    ...overdue.map((title) => [B.task(title).dueDate === B.day(3), `${title} due ${B.day(3)}`]),
    [B.changedOnly(['Return library books', 'Renew the gym membership', 'Cancel the old phone contract']), `changed only Return library books, Renew the gym membership, Cancel the old phone contract (changed: ${B.changedTasks().join(', ')})`],
  ])
}
