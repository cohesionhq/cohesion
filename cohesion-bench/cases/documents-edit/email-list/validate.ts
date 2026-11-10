export default function validate() {
  const [doc] = B.docsLike(/email list/i, B.ANY);
  const text = doc?.contents ?? '';
  return B.check([
    [B.mentions(text, 'priya@example.com', 'sam.lee@example.com', 'helen.carter@example.com'), 'the three addresses'],
    [!B.mentions(text, 'acme.example'), 'no Acme addresses'],
  ])
}
