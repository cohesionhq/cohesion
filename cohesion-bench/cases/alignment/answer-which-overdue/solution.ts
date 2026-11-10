export default function solution() {
  const friday = [1, 2, 3, 4, 5, 6, 7].map((n) => B.date(n)).find((d) => d.getDay() === 5)!;
  B.task('Renew the gym membership').dueDate = B.iso(friday);
  return { asked: ['Which one? [Return library books / Renew the gym membership / Cancel the old phone contract / Expense the conference ticket]'] };
}
