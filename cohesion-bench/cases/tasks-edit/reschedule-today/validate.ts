export default function validate() {
  const today = ['Pick up dry cleaning', 'Check the electricity bill', 'Water the plants'];
  return B.check([
    ...today.map((title) => [B.task(title).dueDate === B.day(1), `${title} due tomorrow`]),
    [B.changedOnly(['Pick up dry cleaning', 'Check the electricity bill', 'Water the plants']), `changed only Pick up dry cleaning, Check the electricity bill, Water the plants (changed: ${B.changedTasks().join(', ')})`],
  ])
}
