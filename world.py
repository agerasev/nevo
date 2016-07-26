#!/usr/bin/python3

from math import sqrt, tanh
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
		return 0.5*sqrt(self.score)

	mass = property(_gmass)
	size = property(_gsize)


class Plant(Entity):
	def __init__(self):
		Entity.__init__(self)

	def proc(self):
		pass


class Animal(Entity):
	def __init__(self):
		Entity.__init__(self)

		self.pot = 0.0
		self.grad = np.zeros(2)

		self.spd = 0.0
		self.mdir = np.zeros(2)

		self.rnn = RNN(3, 3, 4)

	def proc(self):
		self.rnn.i[0] = self.pot
		self.rnn.i[1:3] = self.grad

		self.rnn.proc()

		self.spd = self.rnn.o[0]
		self.mdir = np.copy(self.rnn.o[1:3])


class World:
	def __init__(self):
		self.done = False

		self.size = np.array([640, 640])

		self.entities = {}
		self.access = Lock()
		self.sync = None

		self.idgen = World.IDGen()

		for i in range(10):
			anim = Animal()
			self.entities[next(self.idgen)] = anim
			anim.score = 100
			anim.pos = (np.random.rand(2) - 0.5)*(self.size - 2*anim.size)

	def IDGen():
		counter = 0
		while True:
			counter += 1
			yield counter

	def __call__(self):
		while not self.done:
			with self.access:
				pass
			self.sync()
			sleep(0.040)
