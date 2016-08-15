#pragma once

#include <vector>

#include <core/world.hpp>

#include "entity.hpp"


class MyWorld : public World {
public:
	MyWorld(const vec2 &s) : World(s) {}
	
	std::vector<PG> potential(Entity *e, std::vector<std::function<bool(Entity*)>> selectors) {
		std::vector<PG> pl;
		pl.resize(selectors.size());
		
		for(auto &op : entities) {
			Entity *p = op.second;
			for(int i = 0; i < int(pl.size()); ++i) {
				// double spot = 20*e->size();
				if(selectors[i](p)) { // && length(p->pos - e->pos) - p->size() < spot) {
					vec2 d = p->pos - e->pos;
					double l = length(d) + p->size();
					double m = p->size()/e->size();
					pl[i].pot += m/l;
					pl[i].grad += m*d/(l*l*l);
				}
			}
		}
		
		for(int i = 0; i < int(pl.size()); ++i) {
			double lg = length(pl[i].grad);
			if(lg < 1e-8)
				pl[i].grad = nullvec2;
			else
				pl[i].grad = normalize(pl[i].grad);
		}
		
		return pl;
	}
	
	void sense(Entity *e) override {
		Animal *anim = dynamic_cast<Animal*>(e);
		if(anim == nullptr)
			return;
		
		anim->sense(potential(anim, std::vector<std::function<bool(Entity*)>>({
			[](Entity *e) {return dynamic_cast<Plant*>(e) != nullptr;},
			[anim](Entity *e) {return dynamic_cast<Herbivore*>(e) != nullptr && e != anim;},
			[anim](Entity *e) {return dynamic_cast<Carnivore*>(e) != nullptr && e != anim;}
		})));
	}
};
