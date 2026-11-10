export default function solution() {
  B.task('Buy bin bags').moveTo(B.taskFolder('Errands'));
  B.task('Pay the council tax').moveTo(B.taskFolder('Finance'));
  B.task('Book a physio appointment').moveTo(B.taskFolder('Health'));
  B.task('Descale the shower head').moveTo(B.taskFolder('Home'));
}
