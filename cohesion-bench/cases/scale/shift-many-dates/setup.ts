export default function setup() {
  for (let week = -10; week <= 40; week++) {
    B.ws().createTask({ title: `Weekly report ${week}`, dueDate: B.day(-week) });
  }
}
