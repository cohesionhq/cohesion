export default function validate() {
  const trim = B.newTasks().filter((t) => /trim/i.test(t.title));
  return B.check([[trim.length === 1 && trim[0].dueDate === B.day(7), `a task to trim it, due ${B.day(7)} (the budget is 1500)`]])
}
