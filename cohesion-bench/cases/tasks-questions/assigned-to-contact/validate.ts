export default function validate() {
  return B.answered([
    [B.mentions(reply, 'shelves'), 'hanging the shelves'],
  ])
}
