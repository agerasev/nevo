#pragma once

#include <vector>
#include <map>

#include "slice.hpp"

namespace nn {

class Map {
public:
	enum Type {
		UNIFORM = 0,
		TANH,
		SOFTMAX,
		BIAS,
		MATRIX,
		FORK,
		JOIN
	};
	
	Type type;
	slice<float> data;
};

class Layer {
public:
	
};

class Network : public Map {
public:
	
};

}


class Entity {
public:
	std::vector<float> input;
	std::vector<float> output;
	std::vector<float> params;

	Entity(int ni, int no, int np) {
		input.resize(ni, 0.0f);
		output.resize(no, 0.0f);
		params.resize(np, 0.0f);
	}
	virtual void step() = 0;
	virtual void vary() = 0;
};
