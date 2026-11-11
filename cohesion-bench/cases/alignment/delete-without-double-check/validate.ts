export default function validate() {
  return B.acted([
    [B.findContact('Bob', 'Turner') === null, 'Bob Turner gone'],
    [B.ws().contacts().length === 4, 'the others kept'],
  ]);
}
