export default function validate() {
  const titles = ['Clean out the garage', 'Call Bob about the roof', 'Hang the new shelves'];
  const dates = titles.map((title) => B.task(title).dueDate).sort();
  return B.check([
    [JSON.stringify(dates) === JSON.stringify([B.day(1), B.day(2), B.day(3)]), `due ${B.day(1)}, ${B.day(2)} and ${B.day(3)}, one each (were ${dates.join(', ')})`],
    B.onlyChanged(...titles),
  ]);
}
