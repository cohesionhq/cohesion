const w = cohesion.workspace('Personal');
const make = (title, tag, done) => {
  const task = w.createTask({ title });
  if (tag) task.tag(tag);
  if (done) task.complete();
};
make('Pick up dry cleaning', 'errand');
make('Buy printer ink', 'errand');
make('Return library books', 'errand');
make('Post the parcel', 'errand', true);
make('Write the blog post', 'writing');
make('Call the bank', null);
