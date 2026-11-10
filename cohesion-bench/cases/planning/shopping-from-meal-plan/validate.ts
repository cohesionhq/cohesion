export default function validate() {
  const titles = B.newTasks().filter((t) => t.taskFolderId === B.taskFolder('Errands').id).map((t) => t.title).join(' | ');
  return B.check([
    [/coconut/i.test(titles), 'coconut milk'],
    [/pepper/i.test(titles), 'red pepper'],
    [/feta/i.test(titles), 'feta'],
    [/egg/i.test(titles), 'eggs'],
    [!/lentil/i.test(titles), 'not the lentils, already in the pantry'],
    [!/rice|pasta/i.test(titles), 'nothing not in the plan'],
  ])
}
