#!/usr/bin/python3

import numpy as np


class Mind:
	def __init__(self, *args):
		if isinstance(args[0], Mind):
			mind = args[0]
			self.i = mind.i.copy()
			self.o = mind.o.copy()
			self.w = mind.w.copy()
			self.m = np.zeros_like(mind.m)
		else:
			self.i = np.random.rand(args[0])
			self.o = np.random.rand(args[1])
			self.w = np.random.rand(args[2])
			self.m = np.zeros(args[3])

	def vary(self, delta):
		self.w += delta*np.random.randn(len(self.w))


class RNN(Mind):
	def __init__(self, *args):
		if isinstance(args[0], RNN):
			mind = args[0]
			(ni, no, nh) = (len(mind.i), len(mind.o), len(mind.h))
		else:
			(ni, no, nh) = tuple(args)

		layers = {
			"Wih": (ni, nh),
			"Whh": (nh, nh),
			"bh": (nh,),
			"Who": (nh, no),
			"bo": (no,)
		}

		if isinstance(args[0], RNN):
			Mind.__init__(self, args[0])
		else:
			nw = np.sum([np.prod(v) for v in layers.values()])
			Mind.__init__(self, ni, no, nw, nh)

		self.h = self.m[:]

		ptr = 0
		for name, shape in layers.items():
			size = np.prod(shape)
			value = self.w[ptr:(ptr + size)].reshape(shape)
			self.__setattr__(name, value)
			ptr += size

	def proc(self):
		self.h = np.tanh(
			np.dot(self.Wih, self.i) +
			np.dot(self.Whh, self.h) +
			self.bh)
		self.o = np.dot(self.Who, self.h) + self.bo
