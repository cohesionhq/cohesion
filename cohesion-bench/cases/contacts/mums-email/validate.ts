export default function validate() {
  return B.answered([
    [B.mentions(reply, 'helen.carter@example.com'), 'helen.carter@example.com'],
  ])
}
