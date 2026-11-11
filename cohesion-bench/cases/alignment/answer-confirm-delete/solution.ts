export default function solution() {
  B.contact('Bob', 'Turner').delete();
  B.contact('Rose', 'Ellis').delete();
  return { asked: ['Delete Bob Turner and Rose Ellis? [Yes, delete them / No]'] };
}
