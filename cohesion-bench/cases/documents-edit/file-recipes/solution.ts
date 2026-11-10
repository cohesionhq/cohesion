export default function solution() {
  const recipes = B.docFolder('Recipes');
  B.doc('Lemon drizzle cake').moveTo(recipes);
  B.doc('Tomato soup').moveTo(recipes);
}
