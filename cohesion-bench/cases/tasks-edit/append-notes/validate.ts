export default function validate() {
  const notes = B.task('Plan Lisbon trip').description;
  return B.check([
    [B.mentions(notes, 'Casa do Mar'), 'the hotel noted'],
    [B.mentions(notes, 'Flights'), 'the flights note kept'],
    [B.changedOnly(['Plan Lisbon trip']), `changed only Plan Lisbon trip (changed: ${B.changedTasks().join(', ')})`],
  ])
}
