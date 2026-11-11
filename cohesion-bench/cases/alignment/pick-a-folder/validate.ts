export default function validate() {
  const folder = B.ws().documentFolders().find((f) => f.id === B.doc('Gift ideas').folderId);
  return B.acted([[folder !== undefined && /reference|gift|present/i.test(folder.title), `Gift ideas in Reference or a gifts folder (in ${folder?.title ?? 'the Inbox'})`]]);
}
