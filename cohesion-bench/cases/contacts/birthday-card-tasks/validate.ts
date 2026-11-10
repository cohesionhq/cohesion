export default function validate() {
  const next = (month: number, dayOfMonth: number) => {
    const today = B.date(0);
    let at = new Date(today.getFullYear(), month, dayOfMonth, 12);
    if (at < today) at = new Date(today.getFullYear() + 1, month, dayOfMonth, 12);
    at.setDate(at.getDate() - 7);
    return B.iso(at);
  };
  const due = (name: string) => B.newTasks().find((t) => B.mentions(t.title, name))?.dueDate;
  return B.check([
    [due('Priya') === next(2, 14), `Priya's card due ${next(2, 14)} (was ${due('Priya')})`],
    [due('Sam') === next(7, 2), `Sam's card due ${next(7, 2)} (was ${due('Sam')})`],
    [(due('Helen') ?? due('Mum')) === next(10, 30), `Mum's card due ${next(10, 30)}`],
    [B.newTasks().length === 3, `three tasks (found ${B.newTasks().length})`],
  ])
}
