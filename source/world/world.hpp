#pragma once

#include <functional>
#include <thread>
#include <mutex>
#include <chrono>
#include <map>
#include <cmath>

#include <la/vec.hpp>
#include <la/mat.hpp>

#include "random.hpp"
#include "entity.hpp"
#include "spawn.hpp"


class World {
public:
	long id_counter = 1;
	
	std::map<long, Entity*> entities;
	std::mutex access;
	std::function<void(void)> sync;
	
	bool done = false;
	bool paused = false;
	
	vec2 size;
	
	int delay = 40000; // us
	double step_duration = 0.0; // ms
	long steps_elapsed = 0;
	
	// config
	
	const int max_plant_count = 200;
	int plant_count = 0;
	
	const int min_anim_count = 10;
	int anim_count = 0;
	
	// statistics
	
	long anim_max_age = 0;
	int anim_max_anc = 0;
	
	vec2 rand_pos(double is) {
		return 2*(rand_unif2() - vec2(0.5, 0.5))*(size - vec2(is, is));
	}
	
	void setDelay(int ms) {
		delay = ms;
	}
	
	World(const vec2 &s) {
		size = s;
	}
	~World() {
		for(auto &p : entities) {
			delete p.second;
		}
	}
	
	void add(Entity *e) {
		entities.insert(std::pair<int, Entity*>(id_counter++, e));
	}
	
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
	
	void operator()() {
		auto last_draw_time = std::chrono::steady_clock::now();
		while(!done) {
			if(paused) {
				std::this_thread::sleep_for(std::chrono::milliseconds(40));
				continue;
			}
			
			auto begin_time = std::chrono::steady_clock::now();
			access.lock();
			{
				// interact
				for(auto &p : entities) {
					Entity *e = p.second;
					if(e->active) {
						for(auto &op : entities) {
							Entity *oe = op.second;
							if(length(e->pos - oe->pos) < e->size() + oe->size()) {
								e->interact(oe);
							}
						}
					}
				}
				
				// set inputs
				for(auto &p : entities) {
					Animal *anim = dynamic_cast<Animal*>(p.second);
					if(anim == nullptr)
						continue;
					
					anim->sense(potential(anim, std::vector<std::function<bool(Entity*)>>({
						[](Entity *e) {return dynamic_cast<Plant*>(e) != nullptr;},
						[anim](Entity *e) {return dynamic_cast<Herbivore*>(e) != nullptr && e != anim;},
						[anim](Entity *e) {return dynamic_cast<Carnivore*>(e) != nullptr && e != anim;}
					})));
				}
				
				// process
				for(auto &p : entities) {
					Entity *entity = p.second;
					entity->process();
				}
				
				// remove dead
				for(auto ii = entities.begin(); ii != entities.end();) {
					Entity *e = ii->second;
					if(e->alive) {
						++ii;
					} else {
						if(dynamic_cast<Animal*>(e))
							anim_count -= 1;
						else if(dynamic_cast<Plant*>(e))
							plant_count -= 1;
						entities.erase(ii++);
						delete e;
					}
				}
				
				// reproduce
				for(auto &p : entities) {
					Entity *e = p.second;
					std::list<Entity*> prod = e->produce();
					for(Entity *ne : prod) {
						add(ne);
						if(dynamic_cast<Animal*>(ne))
							anim_count += 1;
						else if(dynamic_cast<Plant*>(ne))
							plant_count += 1;
					}
				}
				
				// move entities
				double dt = 1e-1;
				for(auto &p : entities) {
					Entity *e = p.second;
					e->move(dt);
					
					vec2 msize = size - vec2(e->size(), e->size());
					if(e->pos.x() < -msize.x()) {
						e->pos.x() = -msize.x();
					} else if(e->pos.x() > msize.x()) {
						e->pos.x() = msize.x();
					}
					if(e->pos.y() < -msize.y()) {
						e->pos.y() = -msize.y();
					} else if(e->pos.y() > msize.y()) {
						e->pos.y() = msize.y();
					}
				}
				
				// statistics
				anim_max_age = 0;
				anim_max_anc = 0;
				for(auto &p : entities) {
					Animal *anim = dynamic_cast<Animal*>(p.second);
					if(anim == nullptr)
						continue;
					if(anim_max_age < anim->total_age)
						anim_max_age = anim->total_age;
					if(anim_max_anc < anim->anc)
						anim_max_anc = anim->anc;
				}
			}
			access.unlock();
			
			auto current_time = std::chrono::steady_clock::now();
			auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(current_time - last_draw_time);
			if(duration >= std::chrono::milliseconds(20)) {
				sync();
				last_draw_time = current_time;
			}
			step_duration = 1e-3*std::chrono::duration_cast<std::chrono::microseconds>(current_time - begin_time).count();
			
			steps_elapsed += 1;
			
			std::this_thread::sleep_for(std::chrono::microseconds(delay));
		}
	}
};
