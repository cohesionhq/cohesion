export default function validate() {
  return B.check([[B.findDocFolder('Archive')?.parentId === null, 'a top-level Archive folder']])
}
