export default function validate() {
  return B.answered([
    [B.mentions(reply, 'dry cleaning'), 'the dry cleaning'],
    [B.mentions(reply, 'electricity'), 'the electricity bill'],
    [B.mentions(reply, 'plants'), 'the plants'],
    [!B.mentions(reply, 'library'), 'not the overdue library books'],
  ])
}
