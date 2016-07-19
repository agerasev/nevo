#pragma once

class Entity {
public:
	vector<float> input;
	vector<float> output;
	vector<float> params;

	Entity(int ni, int no, int np) {
		input.resize(ni, 0.0f);
		output.resize(no, 0.0f);
		params.resize(np, 0.0f);
	}
	virtual void step() = 0;
	virtual void vary() = 0;
};

