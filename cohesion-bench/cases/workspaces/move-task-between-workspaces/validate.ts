export default function validate() {
  return B.check([
    [B.tasksLike(/business cards/i).some((t) => t.status === 'incomplete'), 'an open business cards task in Personal'],
    [!B.tasksLike(/business cards/i, 'Acme').some((t) => t.status === 'incomplete'), 'none left open in Acme'],
  ])
}
