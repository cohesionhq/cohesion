export default function validate() {
  return B.check([
    [B.docFolder('Lisbon 2026').parentId === null, 'at the top level'],
    [B.doc('Lisbon itinerary').folderId === B.docFolder('Lisbon 2026').id, 'its documents moved with it'],
  ])
}
