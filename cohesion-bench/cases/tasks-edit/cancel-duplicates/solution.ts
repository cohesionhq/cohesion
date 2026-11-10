export default function solution() {
  for (const title of ['Buy printer ink', 'Call mum', 'Read Piranesi']) {
    B.tasksLike(new RegExp(`^${title}$`)).sort((a, b) => b.id - a.id)[0].cancel();
  }
}
