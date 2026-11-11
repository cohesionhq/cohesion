export default function validate() {
  return B.answered([
    [B.mentions(reply, 'Bob'), 'Bob'],
    [B.mentions(reply, 'Rose'), 'Rose'],
    [!B.mentions(reply, 'Priya'), 'not Priya'],
  ])
}
