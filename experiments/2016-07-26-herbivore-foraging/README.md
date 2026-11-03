# The first NEvo foraging experiment

The first evolutionary version asked whether animals could inherit a movement policy
that found enough food to sustain a lineage. Two recordings from July 2016 document
foraging and a follow-up in which herbivores could also sense one another.

## From a single animal to a population

On July 25, one animal steered directly along a field computed from the positions of
plants and removed plants on contact. On July 26, the C++/Qt simulation gave that
decision to a population of recurrent neural networks. [July 25 prototype](https://github.com/agerasev/nevo/blob/0f8e9af10f950ca7b781d77ccc3274a920c6e180/source/world.hpp)

- **World.** A bounded 2D arena started with 20 random herbivores and 500 mature
  plants. Eaten plants were replaced at random positions and grew back to maturity.
- **Perception and action.** Each animal received the strength of the food field
  and two components of its normalized gradient. Nearby, mature plants contributed
  more. Four recurrent units produced three outputs for movement direction and speed.
- **Selection.** An animal started with 100 energy and spent 2 per step. Eating a
  mature plant supplied 40, enough for 20 steps. Above 400, the parent split its
  energy with a child. The child inherited the 47 controller weights and biases,
  each perturbed by up to 0.02. Animals died when their energy fell below zero.
- **Immigration.** Whenever the population fell below 20, the world introduced
  fresh random brains. Following a lineage through births is therefore more
  informative about successful foraging than counting animals still on screen.

[July 26 world and controller](https://github.com/agerasev/nevo/blob/53d95071148cddc8c53e13ee4e3e1fc6e8f53a63/source/world.hpp) · [Inheritance and mutation](https://github.com/agerasev/nevo/blob/53d95071148cddc8c53e13ee4e3e1fc6e8f53a63/source/entity.hpp)

## What the recordings show

**July 26, Plants and Herbivores (9:19).** Green circles are plants and yellow
circles are animals. The author's description matches the three-input, four-memory-unit
controller above. The arena initially contains many large green circles. At 4:35
and 9:15, open patches and small green specks dominate. The code replenishes plant
count and scales circle size with stored energy. The change in visible coverage
is consistent with grazing replacing mature plants with young ones.
[Start](https://www.youtube.com/watch?v=yCPyA-xxHQA) · [4:35](https://www.youtube.com/watch?v=yCPyA-xxHQA&t=275s) · [9:15](https://www.youtube.com/watch?v=yCPyA-xxHQA&t=555s)

**July 29, Cooperation (10:17).** The author describes adding perception of other
animals and age-dependent death to encourage turnover. In the July 28 source, each
herbivore receives a second potential and gradient from its peers. Aging affects
reproduction and removes older animals with insufficient reserves. Food density,
movement costs, and network size also changed between versions.
[Recording and description](https://www.youtube.com/watch?v=sMux_KPIWoY) · [July 28 rules](https://github.com/agerasev/nevo/blob/44d78a6d46791ebfe624c158d04f95beacdd13ed/source/world.hpp)

One frame reports 28,276 steps, 27 animals, and longest ancestry 57. Around 3:20,
a curved band of herbivores follows the lower edge. Around 3:45–4:05, a band extends
across the interior between an open area and a greener patch. The recording labels
stages at 450,000 and 1,000,000 simulation steps. The ancestry counter records
repeated reproduction. The bands show spatial grouping. Whether sensing neighbors
improved food intake or benefited the group remains an open question.
[Edge group](https://www.youtube.com/watch?v=sMux_KPIWoY&t=200s) · [Interior band](https://www.youtube.com/watch?v=sMux_KPIWoY&t=225s) · [450,000-step stage](https://www.youtube.com/watch?v=sMux_KPIWoY&t=390s)

These observations come from the author's descriptions and published preview frames,
including five-second storyboard samples. The source snapshots explain the rules
of the contemporary versions. The exact builds and seeds of the filmed runs remain
unverified. Carnivores and champion selection belong to the subsequent August work.
