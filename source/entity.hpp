#pragma once

#include <functional>
#include <vector>

class Entity {
public:
	std::vector<float> input;
	std::vector<float> output;
	std::vector<float> weight;
	std::vector<float> memory;

	Entity(int ni, int no, int nw, int nm) {
		input.resize(ni, 0.0f);
		output.resize(no, 0.0f);
		weight.resize(nw, 0.0f);
		memory.resize(nm, 0.0f);
	}
	
	void copy(const Entity &esrc) {
		input = esrc.input;
		output = esrc.output;
		weight = esrc.weight;
		memory.resize(esrc.memory.size(), 0.0f);
	}
	
	Entity(const Entity &esrc) {
		copy(esrc);
	}
	
	Entity &operator =(const Entity &esrc) {
		copy(esrc);
		return *this;
	}
	
	void randomize(std::function<float()> rand) {
		for(int i = 0; i < int(weight.size()); ++i) {
			weight[i] = rand();
		}
	}
	
	void vary(std::function<float()> rand, float delta) {
		for(int i = 0; i < int(weight.size()); ++i) {
			weight[i] += rand()*delta;
		}
	}
};

