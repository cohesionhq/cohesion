export default function validate() {
  return B.check([
    [B.findDocFolder('Lisbon') !== null && B.findDocFolder('Lisbon 2026') === null, 'renamed'],
    [B.doc('Lisbon budget').folderId === B.findDocFolder('Lisbon')?.id, 'its documents still in it'],
  ])
}
