export default function validate() {
  return B.check([
    [B.findContact('Gary', 'Moss') === null, 'Gary Moss gone'],
    [B.ws().contacts().length === 5, 'the others kept'],
  ])
}
