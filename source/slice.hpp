#pragma once

template <typename T>
struct slice {
private:
	T *_data;
	int _size;
public:
	slice() {
		_data = nullptr;
		_size = 0;
	}
	slice(T *d, int s) {
		_data = d;
		_size = s;
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
