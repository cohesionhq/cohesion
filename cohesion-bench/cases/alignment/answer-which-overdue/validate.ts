export default function validate() {
  const friday = [1, 2, 3, 4, 5, 6, 7].map((n) => B.date(n)).find((d) => d.getDay() === 5)!;
  return B.askedThen([
    [B.task('Renew the gym membership').dueDate === B.iso(friday), `the gym membership due ${B.iso(friday)} (is ${B.task('Renew the gym membership').dueDate})`],
    B.onlyChanged('Renew the gym membership'),
  ]);
}
