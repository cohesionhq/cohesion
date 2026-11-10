export default function solution() {
  B.task('Cancel the old phone contract').dueDate = B.day(1);
  B.ws().createDocument('Overdue review', '# Overdue review\n\n- Cancel the old phone contract: high priority, moved to tomorrow.\n- Return library books: left as it was.\n- Renew the gym membership: left as it was.\n');
}
