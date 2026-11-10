export default function validate() {
  const where = (pattern: RegExp) => B.tasksLike(pattern)[0]?.taskFolderId;
  return B.check([
    [where(/parcel/i) === B.taskFolder('Errands').id, 'the parcel in Errands'],
    [where(/council tax/i) === B.taskFolder('Finance').id, 'council tax in Finance'],
    [where(/physio/i) === B.taskFolder('Health').id, 'physio in Health'],
    [where(/shower/i) === B.taskFolder('Home').id, 'shower head in Home'],
  ])
}
