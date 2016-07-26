#pragma once

#include <cmath>

#ifdef _VASSERT
#include <cassert>
#define _vassert assert
#else
#define _vassert(arg) \
	do{}while(false)
#endif

template <typename T>
struct slice {
public:
	T *_data;
	int _size;

	slice() {
		_data = nullptr;
		_size = 0;
	}
	slice(T *d, int s) {
		_data = d;
		_size = s;
	}
	
	inline T &operator [](int p) {
		_vassert(p >= 0 && p < _size);
		return _data[p];
	}
	inline const T &operator [](int p) const {
		_vassert(p >= 0 && p < _size);
		return _data[p];
	}

	T *data() {
		return _data;
	}
	const T *data() const {
		return _data;
	}
	int size() const {
		return _size;
	}
};

template <typename T>
struct vector : public slice<T> {
public:
	vector() : slice<T>() {}
	vector(int s) : slice<T>(new T[s], s) {}
	~vector() {
		if(this->_data != nullptr) {
			delete[] this->_data;
		}
	}
};

template <typename T>
void copy(slice<T> &o, const slice<T> &a) {
	_vassert(o.size() == a.size());
	for(int i = 0; i < o.size(); ++i) {
		o[i] = a[i];
	}
}

template <typename T>
void add(slice<T> &o, const slice<T> &a, const slice<T> &b) {
	_vassert(o.size() == a.size());
	_vassert(o.size() == b.size());
	for(int i = 0; i < o.size(); ++i) {
		o[i] = a[i] + b[i];
	}
}

template <typename T>
void dot(slice<T> &o, const slice<T> &m, const slice<T> &a) {
	_vassert(o.size()*a.size() == m.size());
	int w = a.size();
	for(int i = 0; i < o.size(); ++i) {
		o[i] = 0.0;
		for(int j = 0; j < w; ++j) {
			o[i] += m[i*w + j]*a[j];
		}
	}
}

template <typename T>
void tanh(slice<T> &o, const slice<T> &a) {
	_vassert(o.size() == a.size());
	for(int i = 0; i < o.size(); ++i) {
		o[i] = tanh(a[i]);
	}
}
