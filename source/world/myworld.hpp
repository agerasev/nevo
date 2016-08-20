#pragma once

#include <vector>

#include <core/world.hpp>

#include "organism.hpp"


class MyWorld : public World {
public:
	MyWorld(const vec2 &s) : World(s) {}
	
	std::vector<PG> potential(Organism *e, std::vector<std::function<bool(Organism*)>> selectors) {
		std::vector<PG> pl;
		pl.resize(selectors.size());
		
		for(auto &op : entities) {
			Organism *p = static_cast<Organism*>(op.second);
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
	
	void sense(Organism *e) {
		Animal *anim = dynamic_cast<Animal*>(e);
		if(anim == nullptr)
			return;
		
		anim->sense(potential(anim, std::vector<std::function<bool(Organism*)>>({
			[](Organism *e) {return dynamic_cast<Plant*>(e) != nullptr;},
			[anim](Organism *e) {return dynamic_cast<Herbivore*>(e) != nullptr && e != anim;},
			[anim](Organism *e) {return dynamic_cast<Carnivore*>(e) != nullptr && e != anim;}
		})));
	}
	
	void process() {
		for(auto &p : entities) {
			static_cast<Organism*>(p.second)->process();
		}
	}
	
	void remove_dead() {
		for(auto ii = entities.begin(); ii != entities.end();) {
			Organism *e = static_cast<Organism*>(ii->second);
			if(e->alive) {
				++ii;
			} else {
				entities.erase(ii++);
				delete e;
			}
		}
	}
	
	void reproduce() {
		for(auto &p : entities) {
			Organism *e = static_cast<Organism*>(p.second);
			std::list<Organism*> prod = e->produce();
			for(Organism *ne : prod) {
				add(ne);
			}
		}
	}
	
	void step() override {
		interact();
		
		// sense
		for(auto &p : entities) {
			sense(static_cast<Organism*>(p.second));
		}
		
		process();
		
		reproduce();
		
		remove_dead();
		
		move();
	}
};
