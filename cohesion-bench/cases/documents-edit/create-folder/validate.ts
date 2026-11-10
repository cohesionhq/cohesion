export default function validate() {
  return B.check([[B.findDocFolder('Archive', B.ANY)?.parentId === null, 'a top-level Archive folder']])
}
