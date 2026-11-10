export default function validate() {
  const folder = B.ws().documentFolders().find((f) => f.id === B.doc('Lisbon budget').folderId);
  return B.acted([
    [B.findDocFolder('Lisbon 2026') === null, 'Lisbon 2026 renamed'],
    [folder !== undefined && folder.title.trim().length > 0 && folder.title.length < 'Lisbon 2026'.length, `a shorter name (now ${folder?.title})`],
  ]);
}
