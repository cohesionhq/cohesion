export default function validate() {
  const overdue = ['Return library books', 'Renew the gym membership', 'Cancel the old phone contract'];
  return B.acted([
    ...overdue.map((title) => [B.task(title).status === 'cancelled', `${title} cancelled`]),
    B.onlyChanged(...overdue),
  ]);
}
