export default function validate() {
  const errands = ['Buy printer ink', 'Pick up dry cleaning', 'Return library books', 'Get a spare house key cut', 'Buy milk'];
  return B.check([
    ...errands.map((title) => [B.task(title).status === 'completed', `${title} done`]),
    B.onlyChanged(...errands),
  ]);
}
