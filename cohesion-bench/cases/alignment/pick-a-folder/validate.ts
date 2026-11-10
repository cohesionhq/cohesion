export default function validate() {
  return B.acted([[B.doc('Gift ideas').folderId !== null, 'Gift ideas in a folder']]);
}
