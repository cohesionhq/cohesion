const w = cohesion.workspace('Personal');
w.createTaskFolder({ title: 'Errands' });
w.createTaskFolder({ title: 'Work' });
for (const title of ['Buy milk', 'Buy eggs and bread', 'Get coffee beans', 'Fix the login bug', 'Review Sam\'s pull request']) {
  w.createTask({ title });
}
