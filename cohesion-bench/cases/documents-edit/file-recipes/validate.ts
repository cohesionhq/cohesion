export default function validate() {
  const recipes = B.docFolder('Recipes').id;
  return B.check([
    [B.doc('Lemon drizzle cake').folderId === recipes, 'the cake recipe filed'],
    [B.doc('Tomato soup').folderId === recipes, 'the soup recipe filed'],
    [B.doc('Gift ideas').folderId === null && B.doc('Reading list').folderId === null && B.doc('Garden plan').folderId === null, 'the other Inbox documents left'],
  ])
}
