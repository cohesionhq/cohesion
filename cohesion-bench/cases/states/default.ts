// The default starting state: a lived-in Personal workspace and an Acme work
// one, as plain data (StartingState in types/bench.d.ts and
// src/starting_state.rs). Days are counted from today, so -1 is yesterday.
// Cases rely on these titles and facts: change one and run
// `cohesion-bench validate`.
export default {
  workspaces: [
    {
      title: 'Personal',
      tags: ['errand', 'phone', 'family', 'home-repair', 'waiting'],
      taskFolders: ['Home', 'Errands', 'Health', 'Finance', 'Work'],
      contacts: [
        { firstName: 'Priya', lastName: 'Shah', methods: [{ kind: 'email', value: 'priya@example.com' }, { kind: 'phone', value: '+44 7700 900111' }], notes: 'Birthday: 14 March. Partner: Dev. Allergic to peanuts.' },
        { firstName: 'Sam', lastName: 'Lee', methods: [{ kind: 'email', value: 'sam.lee@example.com' }], notes: 'Neighbour at number 12. Birthday: 2 August. Daughter: Mia.' },
        { firstName: 'Bob', lastName: 'Turner', methods: [{ kind: 'phone', value: '+44 7700 900222' }], notes: 'Roofer. Quoted £1,200 for the flashing in September.' },
        { firstName: 'Helen', lastName: 'Carter', methods: [{ kind: 'phone', value: '+44 7700 900333' }, { kind: 'email', value: 'helen.carter@example.com' }], notes: 'Mum. Birthday: 30 November. Likes gardening and crime novels.' },
        { firstName: 'Rose', lastName: 'Ellis', methods: [{ kind: 'phone', value: '+44 7700 900444' }], notes: 'Grandma. Lives in Whitby. No email.' },
      ],
      tasks: [
        { title: 'Fix the leaking kitchen tap', folder: 'Home', priority: 'high', size: 2, due: 2, tags: ['home-repair'] },
        { title: 'Clean out the garage', folder: 'Home', priority: 'low', size: 5 },
        { title: 'Repaint the fence', folder: 'Home', size: 4, start: 10, due: 30, tags: ['home-repair'] },
        { title: 'Replace the hallway light bulb', folder: 'Home', size: 1, status: 'completed' },
        { title: 'Call Bob about the roof', folder: 'Home', priority: 'medium', tags: ['phone', 'home-repair'] },
        { title: 'Hang the new shelves', folder: 'Home', size: 2, assignee: { kind: 'contact', firstName: 'Sam', lastName: 'Lee' } },
        { title: 'Buy printer ink', folder: 'Errands', tags: ['errand'] },
        { title: 'Pick up dry cleaning', folder: 'Errands', due: 0, tags: ['errand'] },
        { title: 'Return library books', folder: 'Errands', due: -2, tags: ['errand'] },
        { title: 'Post the parcel to Grandma', folder: 'Errands', tags: ['errand'], status: 'completed' },
        { title: 'Get a spare house key cut', folder: 'Errands', size: 1, tags: ['errand'] },
        { title: 'Buy milk', folder: 'Errands' },
        { title: 'Book the dentist', folder: 'Health', priority: 'high', due: 5, tags: ['phone'] },
        { title: 'Renew the gym membership', folder: 'Health', priority: 'medium', due: -5 },
        { title: 'Order contact lenses', folder: 'Health', size: 1 },
        { title: 'Get a flu jab', folder: 'Health', due: 14 },
        { title: 'Start the Couch to 5k plan', folder: 'Health', status: 'cancelled' },
        { title: 'File the tax return', folder: 'Finance', priority: 'urgent', size: 4, due: 40, description: 'Need the P60 and the pension statements before starting.' },
        { title: 'Cancel the old phone contract', folder: 'Finance', priority: 'high', due: -1, tags: ['phone'] },
        { title: 'Check the electricity bill', folder: 'Finance', due: 0 },
        { title: 'Set up the ISA transfer', folder: 'Finance', status: 'completed' },
        { title: 'Claim back the train fare', folder: 'Finance', size: 1, due: 9 },
        { title: 'Update my CV', folder: 'Work', size: 3 },
        { title: 'Buy a new kettle', folder: 'Work' },
        { title: 'Plan Lisbon trip', due: 20, description: 'Flights: not booked yet.\nHotel: undecided.' },
        { title: 'Read Piranesi', due: 12 },
        { title: 'Sign up for the Spanish evening course', priority: 'low' },
        { title: 'Call mum', tags: ['phone', 'family'] },
        { title: 'Water the plants', size: 1, due: 0 },
        { title: 'Write thank-you cards', priority: 'low', size: 2 },
        { title: 'Fix the bike puncture', description: 'Tyre size is 700x28c. A spare inner tube is in the shed.' },
        { title: 'Book the car service', description: 'The car is due its 60,000-mile service. The garage is Smith Motors; ask for Jo.' },
        { title: 'Hear back from the landlord', tags: ['waiting'] },
      ],
      documentFolders: [
        {
          title: 'Recipes',
          documents: [
            { title: 'Banana bread', contents: `# Banana bread

Serves 8. Takes 70 minutes.

## Ingredients

- 3 ripe bananas
- 250 g plain flour
- 100 g butter
- 150 g brown sugar
- 2 eggs
- 1 tsp baking soda

## Method

1. Mash the bananas.
2. Mix everything.
3. Bake at 175°C for 60 minutes.
` },
            { title: 'Weeknight dal', contents: `# Weeknight dal

Serves 4. Takes 35 minutes.

## Ingredients

- 200 g red lentils
- 1 onion
- 2 cloves garlic
- 1 tin coconut milk
- 2 tsp curry powder
` },
            { title: 'Shakshuka', contents: `# Shakshuka

Serves 2. Takes 30 minutes.

## Ingredients

- 4 eggs
- 1 tin chopped tomatoes
- 1 red pepper
- 1 tsp cumin
- Feta, to serve
` },
            { title: 'Green salad', contents: `# Green salad

Serves 2. Takes 10 minutes.

## Ingredients

- 1 head of lettuce
- 1 cucumber
- Olive oil and lemon
` },
          ],
        },
        {
          title: 'Trips',
          folders: [
            {
              title: 'Lisbon 2026',
              documents: [
                { title: 'Lisbon itinerary', contents: `# Lisbon itinerary

## Day 1

Arrive, check in, walk around Alfama.

## Day 2

Belém in the morning, LX Factory in the afternoon.

## Day 3

Take the ferry to Cacilhas for lunch, then the Gulbenkian.
` },
                { title: 'Lisbon budget', contents: `# Lisbon budget

| Item | Cost (EUR) |
|---|---|
| Flights | 320 |
| Hotel | 690 |
| Food | 350 |
| Transport | 80 |
| Museums | 60 |
` },
              ],
            },
          ],
          documents: [
            { title: 'Packing list', contents: `# Packing list

- [ ] Passport
- [ ] Phone charger
- [ ] Sunscreen
- [ ] Swimsuit
- [ ] Walking shoes
- [x] Adapter plug
` },
          ],
        },
        {
          title: 'Meetings',
          documents: [
            { title: 'Kickoff meeting notes', contents: `# Kickoff

Attendees: Ana, Bo

## Decisions

- Launch the beta on 1 December
- Weekly check-ins on Tuesdays
- Ana owns the launch checklist
` },
            { title: 'Ops sync', contents: `# Ops sync

## Notes

- The backup job failed twice last week.
- Disk usage on the media server is at 91%.

## Action items

- Me: rotate the backup credentials by 2026-12-05
- Me: order two 8 TB drives by 2026-12-08
- Bo: write the incident summary by 2026-12-04
` },
            { title: 'Garden committee', contents: `# Garden committee

We agreed to plant the wildflower strip in spring and to move the compost bins behind the shed. The next meeting is in January.
` },
          ],
        },
        {
          title: 'Reference',
          documents: [
            { title: 'Cabin', contents: `# The cabin

The key is under the blue pot by the door.

Wifi: network LakeView, password pinecone-42.

Bins go out on Thursday evenings.
` },
            { title: 'Car', contents: `# Car

Make: Skoda Octavia estate, 2019.
Registration plate: KX21 BNM.
MOT due: every March.
Insurer: Harbour Mutual, policy CAR-77310.
` },
            { title: 'Home insurance', contents: `# Home insurance

Insurer: Northgate.
Policy number: HX-448812.
Renews every 1 February. Excess: £250.
` },
            { title: 'Pantry staples', contents: `# Pantry staples

- Rice
- Pasta
- Tinned tomatoes
- Red lentils
- Coffee beans
` },
          ],
        },
      ],
      documents: [
        { title: 'Reading list', contents: `# Reading list

- Piranesi
- The Overstory
- Klara and the Sun
` },
        { title: 'Gift ideas', contents: `# Gift ideas

- Mum: a gardening kneeler, or the new Ann Cleeves novel
- Sam: a board game for Mia
- Priya: anything but peanuts; maybe a cookbook
` },
        { title: 'Garden plan', contents: `# Garden plan

## Beds

Tomatoes and beans in the south bed. Herbs by the kitchen door.

## Old ideas

A pond by the fence. A second shed.

## Budget

About £300 for the year.
` },
      ],
      journals: [
        { day: -1, contents: 'Ran 5k in 31 minutes. Dinner with Priya at the Thai place.' },
        { day: -2, contents: 'Rainy all day. Read the first four chapters of Piranesi.' },
        { day: -3, contents: 'Fixed the wobbly shelf in the hallway. Called Grandma.' },
      ],
    },
    {
      title: 'Acme',
      taskFolders: ['Sprint 42', 'Backlog'],
      contacts: [
        { firstName: 'Mei', lastName: 'Chen', methods: [{ kind: 'email', value: 'mei@acme.example' }], notes: 'Engineering manager. 1:1 every other Thursday.' },
        { firstName: 'Leo', lastName: 'Park', methods: [{ kind: 'email', value: 'leo@acme.example' }, { kind: 'phone', value: '+1 415 555 0142' }], notes: 'Backend engineer, owns billing.' },
      ],
      tasks: [
        { title: 'Fix the login redirect bug', folder: 'Sprint 42', priority: 'urgent', size: 2, due: 1 },
        { title: "Review Leo's pull request", folder: 'Sprint 42', priority: 'high', due: 0 },
        { title: 'Write release notes for 3.2', folder: 'Sprint 42', size: 2, due: 2 },
        { title: 'Pair with Mei on the billing export', folder: 'Sprint 42', assignee: { kind: 'contact', firstName: 'Mei', lastName: 'Chen' } },
        { title: 'Upgrade the Postgres cluster', folder: 'Backlog', size: 5 },
        { title: 'Add dark mode to settings', folder: 'Backlog', size: 3 },
        { title: 'Document the deploy process', folder: 'Backlog', size: 2 },
        { title: 'Expense the conference ticket', due: -3 },
        { title: 'Order business cards' },
      ],
      documents: [
        { title: 'API spec', contents: `# API spec

## Authentication

Every request carries a bearer token.

## Re-auth

When a token expires the client calls /v2/session/refresh with its refresh token; after three failures it signs the user out.

## Limits

Each key may make 600 requests per minute. Bulk endpoints allow 20.
` },
        { title: 'Falcon project brief', contents: `# Falcon project brief

Falcon replaces the nightly CSV export with a streaming billing feed. Owner: Leo Park. Target: end of Q1.
` },
        { title: '1:1 with Mei', contents: `# 1:1 with Mei

- Wants the deploy process written down before the holidays.
- Asked me to mentor the new graduate, Tom.
- Promotion case review is in February.
` },
        { title: 'Onboarding guide', contents: `# Onboarding guide

On your first day you will recieve a laptop and a badge.

You should recieve an email with your accounts by noon.

If you do not recieve it, ask in #it-help.
` },
        { title: 'Launch plan', contents: `# Launch plan

## Goals

Ship 3.2 to every customer by the end of the month.

## Timeline

- Code freeze on the 20th
- Release on the 27th
` },
      ],
    },
  ],
} satisfies StartingState;
