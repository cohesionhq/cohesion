export default function validate() {
  return B.acted([
    [B.findDocFolder('Recipes') === null, 'Recipes gone'],
    [B.docsLike(/banana bread|weeknight dal|shakshuka|green salad/i).length === 4, 'the four recipes kept'],
  ]);
}
