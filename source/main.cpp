#include <cstdio>
#include <vector>

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

int main(int argc, char *argv[]) {
	
	return 0;
}
