export default function validate() {
  return B.askedThen([
    [B.task('Book the dentist').dueDate === B.day(9), `due ${B.day(9)} (is ${B.task('Book the dentist').dueDate})`],
    B.onlyChanged('Book the dentist'),
  ]);
}
