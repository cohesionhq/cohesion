export default function validate() {
  const folder = B.docFolder('Lisbon 2026').id;
  const places = ['Alfama', 'Belém', 'Cacilhas'];
  const checks = places.map((place, i) => {
    const [doc] = B.docsLike(new RegExp(`^lisbon day ${i + 1}$`, 'i'));
    const others = places.filter((p) => p !== place);
    return [
      doc?.folderId === folder && B.mentions(doc.contents, place) && others.every((p) => !B.mentions(doc.contents, p)),
      `Lisbon day ${i + 1} in Lisbon 2026, with ${place} and no other day`,
    ];
  });
  return B.check([
    ...checks,
    [B.mentions(B.doc('Lisbon itinerary').contents, ...places), 'the itinerary kept'],
  ]);
}
