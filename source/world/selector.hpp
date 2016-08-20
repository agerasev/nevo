#pragma once

#include <list>

#include "random.hpp"

#include "organism.hpp"

#include "mind.hpp"

struct Champion {
	double score;
	Mind mind;
	
	Champion(double s, const Mind &m) : score(s), mind(m) {}
};

bool operator < (const Champion &a, const Champion &b) {
	return a.score < b.score;
}

class Selector {
public:
	std::list<Champion> champions;
	double min_score = 0.0;
	double max_score = 0.0;
	
	const int champions_max_count = 16;
	
	void add(Animal *a) {
		float score = a->score();
		if(score > min_score) {
			auto it = champions.begin();
			while(it->score < score && it != champions.end()) {
				++it;
			}
			champions.insert(it, Champion(score, a->mind));
		}
	}
	
	void select() {
		while(int(champions.size()) > champions_max_count) {
			champions.pop_front();
		}
		for(auto &c : champions) {
			c.score *= 1.0 - 1e-4;
		}
		if(champions.size()) {
			min_score = champions.front().score;
			max_score = champions.back().score;
		}
	}
	
	const Mind *genMind() const {
		if(champions.size() && rand_unif() > 0.5) {
			int rp = rand_int() % champions.size();
			auto it = champions.begin();
			for(int i = 0; i < rp; ++i) {
				++it;
			}
			return &it->mind;
		} else {
			return nullptr;
		}
	}
};
