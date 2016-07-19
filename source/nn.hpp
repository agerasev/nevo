#pragma once

#include <vector>
#include <map>

using namespace std;

#include "slice.hpp"

namespace nn {

class Layer {
private:
	vector<float> _vector;
public:
	int size;
	float *data;

	Layer(int s) {
		size = s;
		_vector.resize(size);
		data = _vector.data();
	}
};

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
private:
	slice<float> _slice;
	vector<Layer*> _in, _out;
public:
	Type type;
	int size;
	float *data;
	Layer **in, **out;

	Map(Type t, int s = 0, float *d = nullptr) : _slice(d, s) {
		type = t;
		size = s;
		data = _slice.data();
	}

	void bind(Layer **i, Layer **o) {
		if(type == JOIN) {
			_in.resize(2);
			in = _in.data();
			in[0] = i[0];
			in[1] = i[1];
		} else {
			_in.resize(1);
			in = _in.data();
			*in = *i;
		}

		if(type == FORK) {
			_out.resize(2);
			out = _out.data();
			out[0] = i[0];
			out[1] = i[1];
		} else {
			_out.resize(1);
			out = _out.data();
			*out = *i;
		}
	}

	void forward() {
		switch(type) {
		case: UNIFORM:

		}
	}
};

class Network : public Map {
public:
	
};

}
