export default function validate() {
  const [doc] = B.docsLike(/errands list/i, B.ANY);
  const text = doc?.contents ?? '';
  return B.check([
    [B.mentions(text, 'printer ink', 'dry cleaning', 'library books', 'house key'), 'the four open errands'],
    [!B.mentions(text, 'parcel'), 'not the posted parcel'],
  ])
}
