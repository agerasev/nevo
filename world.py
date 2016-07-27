#!/usr/bin/python3

from math import sin, cos, sqrt, tanh, pi
from threading import Lock
from time import sleep

import numpy as np

from mind import RNN


class Entity:
	def __init__(self):
		self.alive = True
		self.score = 0.0

		self.pos = np.zeros(2)
		self.vel = np.zeros(2)

	def proc():
		raise NotImplementedError()

	def _gmass(self):
		return self.score

	def _gsize(self):
		if self.score < 0.0:
			return 0.0
		return 0.5*sqrt(self.score)

	mass = property(_gmass)
	size = property(_gsize)


class Plant(Entity):
	def __init__(self):
		Entity.__init__(self)

	def proc(self):
		pass


class Animal(Entity):
	def __init__(self, anim=None):
		Entity.__init__(self)

		self.pot = 0.0
		self.grad = np.zeros(2)

		self.spd = 0.0
		self.mdir = np.zeros(2)

		if anim is None:
			self.rnn = RNN(3, 3, 4)
		else:
			self.rnn = RNN(anim.rnn)

	def proc(self):
		self.rnn.i[0] = self.pot
		self.rnn.i[1:3] = self.grad

		# self.rnn.proc()

		self.spd = self.rnn.o[0]
		self.mdir = np.copy(self.rnn.o[1:3])


class Timer:
	def __init__(self, period, callback):
		self.period = period
		self.callback = callback
		self.counter = 0

	def proc(self, step=1):
		self.counter += step
		while self.counter >= self.period:
			self.counter -= self.period
			self.callback()


class World:
	def __init__(self):
		self.done = False

		self.size = np.array([640, 640])

		self.entities = {}
		self.access = Lock()
		self.sync = None

		self.idgen = World.IDGen()

		self.delay = 40.0

		self.config = {
			"animal": {
				"min_count": 20,
				"max_speed": 100.0,
				"feed_factor": 0.1,
				"time_fine": 2.0,
				"move_fine": 0.0,
				"breed_threshold": 400.0,
				"init_score": 100.0
			},
			"plant": {
				"max_count": 500,
				"max_score": 400.0,
				"grow_speed": 1.0
			}
		}

		self.animal_count = 0
		self.plant_count = 0

		for i in range(self.config["animal"]["min_count"]):
			self.spawn_animal()

		for i in range(self.config["plant"]["max_count"]):
			self.spawn_plant()

	def IDGen():
		counter = 0
		while True:
			counter += 1
			yield counter

	def spawn_animal(self):
		anim = Animal()

		anim.score = self.config["animal"]["init_score"]
		anim.pos = self.rand_pos(anim.size)

		self.entities[next(self.idgen)] = anim
		self.animal_count += 1

	def breed_animal(self, asrc):
		anim = Animal(asrc)

		asrc.score /= 2
		anim.score = asrc.score

		ang = 2*pi*np.random.rand()
		bdir = np.array([sin(ang), cos(ang)])
		anim.pos = asrc.pos + 0.5*bdir*asrc.size
		asrc.pos -= 0.5*bdir*asrc.size

		anim.rnn.vary(0.04)

		self.entities[next(self.idgen)] = anim
		self.animal_count += 1

	def spawn_plant(self):
		plant = Plant()
		plant.score = 0.0
		plant.pos = self.rand_pos(0)

		self.entities[next(self.idgen)] = plant
		self.plant_count += 1

	def potential(self, pos, selector):
		pot = 0.0
		grad = np.zeros(2)
		count = 0
		for entity in self.entities.values():
			if selector(entity):
				count += 1
				d = 0.5*(pos - entity.pos)/np.min(self.size)
				l = np.linalg.norm(d) + 1e-4
				m = entity.score
				pot += m/l
				grad += m*d/l**3
		if(count > 0):
			pot /= count
		l = np.linalg.norm(grad)
		if(l > 1e-4):
			grad /= l
		else:
			grad = np.zeros(0, 0)
		return (pot, grad)

	def feed(self, animal):
		for plant in self.entities.values():
			if not isinstance(plant, Plant):
				continue
			if not plant.alive:
				continue
			l = np.linalg.norm(plant.pos - animal.pos)
			ml = 0.8*(plant.size + animal.size)
			if l < ml:
				plant.alive = False
				self.plant_count -= 1
				animal.score += plant.score*self.config["animal"]["feed_factor"]

	def __call__(self):
		while not self.done:
			with self.access:

				# consume

				for animal in self.entities.values():
					if isinstance(animal, Animal):
						# eat plants
						self.feed(animal)
						# die
						if animal.score < 0.0:
							animal.alive = False
							self.animal_count -= 1

				delkeys = []
				for key, entity in self.entities.items():
					if not entity.alive:
						delkeys.append(key)
				for key in delkeys:
					del self.entities[key]

				# produce

				while self.plant_count < self.config["plant"]["max_count"]:
					self.spawn_plant()

				while self.animal_count < self.config["animal"]["min_count"]:
					self.spawn_animal()

				for animal in self.entities.values():
					if not isinstance(animal, Animal):
						continue
					if animal.score > self.config["animal"]["breed_threshold"]:
						self.breed_animal(animal)

				# update scores
				for entity in self.entities.values():
					if isinstance(entity, Animal):
						time_fine = self.config["animal"]["time_fine"]
						move_fine = self.config["animal"]["move_fine"]
						move_fine *= np.linalg.norm(entity.vel)/self.config["animal"]["max_speed"]
						entity.score -= time_fine + move_fine
					elif isinstance(entity, Plant):
						max_score = self.config["plant"]["max_score"]
						if entity.score < max_score:
							entity.score += self.config["plant"]["grow_speed"]
							if entity.score > max_score:
								entity.score = max_score

				# set inputs
				for animal in self.entities.values():
					if not isinstance(animal, Animal):
						continue
					pot, grad = self.potential(animal.pos, lambda p: isinstance(p, Plant))
					animal.pot = pot/self.config["plant"]["max_score"]
					animal.grad = grad

				# process
				for entity in self.entities.values():
					entity.proc()

				# get output
				for entity in self.entities.values():
					if not isinstance(animal, Animal):
						continue
					spd = self.config["animal"]["max_speed"]*abs(tanh(entity.spd))
					mdl = np.linalg.norm(entity.mdir)
					if(mdl > 1e-4):
						animal.vel = spd*self.mdir/mdl
					else:
						animal.vel = np.zeros(2)

				# move items
				dt = 1e-1
				for entity in self.entities.values():
					entity.pos += entity.vel*dt
					mpos = self.size - entity.size
					np.clip(entity.pos, -mpos, mpos)

				self.sync()
			sleep(1e-3*self.delay)

	def rand_pos(self, size):
		return (np.random.rand(2) - 0.5)*(self.size - 2*size)
