export default function validate() {
  const saturday = [1, 2, 3, 4, 5, 6, 7].find((n) => B.date(n).getDay() === 6)!;
  const want: [string, number][] = [['Wash the car', saturday + 2], ['Batch cook for the week', saturday + 2], ['Mow the lawn', saturday + 9]];
  return B.check([
    ...want.map(([title, n]) => [B.task(title).dueDate === B.day(n), `${title} due ${B.day(n)} (is ${B.task(title).dueDate})`]),
    B.onlyChanged(...want.map(([title]) => title)),
  ]);
}
