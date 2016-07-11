#pragma once

#include <vector>
#include <map>

#include "slice.hpp"

namespace nn {

class Node {
public:
	std::vector<std::vector<float>*> ibufs, obufs;
	
	Node(int ni, int no) {
		ib.resize(ni);
		ob.resize(no);
	}
	virtual void ilink(std::vector<float> *buf, int p = 0) {
		ibufs[p] = buf;
	}
	virtual void olink(std::vector<float> *buf, int p = 0) {
		obufs[p] = buf;
	}
};

class Network : public Node {
public:
	std::vector<Node*> nodes;
	Network(int ni, int no) : Node(ni, no) {
		
	}
	virtual void ilink(std::vector<float> *buf, int p = 0) override {
		Node::ilink(buf, p);
	}
	virtual void olink(std::vector<float> *buf, int p = 0) override {
		Node::olink(buf, p);
	}
};

}
