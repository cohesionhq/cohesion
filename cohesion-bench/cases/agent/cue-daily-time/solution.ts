export default function solution() {
  return ({ cues: [{ name: 'Water the plants', hook: 'timer', code: '({ hour, minute }) => hour === 18 && minute === 0', instructions: 'Remind the user to water the plants.', effort: 'low' }] });
}
