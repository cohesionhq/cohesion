export default function validate() {
  return B.askedThen([
    [B.findContact('Bob', 'Turner') === null && B.findContact('Rose', 'Ellis') === null, 'both gone'],
    [B.ws().contacts().length === 3, 'the others kept'],
  ]);
}
