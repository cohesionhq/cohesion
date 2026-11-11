export default function validate() {
  return B.check([[B.doc('Gift ideas').folderId === B.docFolder('Reference').id, 'Gift ideas in Reference']])
}
