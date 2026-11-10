export default function validate() {
  const copies = B.docsLike(/packing list/i, 'Acme');
  return B.check([
    [copies.length === 1 && copies[0].contents.trim() === B.doc('Packing list').contents.trim(), 'the same Packing list in Acme'],
    [B.docsLike(/packing list/i).length === 1, 'the original kept'],
  ]);
}
