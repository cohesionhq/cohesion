export default function validate() {
  const made = B.newTasks(B.ANY);
  const about = (pattern: RegExp) => made.filter((t) => pattern.test(t.title));
  return B.check([
    [about(/flight/i).length === 1 && about(/flight/i)[0].dueDate === B.day(10), `a flights task due ${B.day(10)}`],
    [about(/hotel/i).length === 1 && about(/hotel/i)[0].dueDate === B.day(10), `a hotel task due ${B.day(10)}`],
    [about(/food/i).length === 1 && about(/food/i)[0].dueDate === B.day(10), `a food task due ${B.day(10)}`],
    [made.length === 3, `only those three (made ${made.length})`],
  ]);
}
