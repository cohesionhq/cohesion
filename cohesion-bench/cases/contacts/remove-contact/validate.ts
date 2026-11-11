export default function validate() {
  return B.check([
    [B.findContact('Rose', 'Ellis') === null, 'Rose Ellis gone'],
    [B.ws().contacts().length === 4, 'the others kept'],
  ]);
}
