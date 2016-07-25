#pragma once

#include <functional>
#include <thread>
#include <mutex>
#include <chrono>
#include <map>
#include <cmath>
#include <random>

#include <la/vec.hpp>
#include <la/mat.hpp>

#include "entity.hpp"


class Item {
public:
	int type;
	
	bool exists = true;
	
	double mass = 1.0;
	vec2 pos = nullvec2;
	vec2 vel = nullvec2;
	
	double size = 10;
	
	Item(int t) {
		type = t;
	}
	virtual ~Item() = default;
	
	virtual void proc() = 0;
};

class Plant : public Item {
public:
	Plant() : Item(0) {
		
	}
	
	virtual void proc() override {
		
	}
};

class Animal : public Item {
public:
	const int 
		ni = 3,
		no = 3;
		// nh = 8;
	
	Entity entity;
	
	Animal() : 
		Item(1),
		entity(ni, no, 0) 
		// ni*nh + (nh*nh + nh) + nh*no + no
	{
		
	}
	
	virtual void proc() override {
		entity.output[0] = entity.input[0];
		entity.output[1] = entity.input[1];
		entity.output[2] = 1.0;
	}
};

class World {
public:
	int id_counter = 1;
	
	std::map<int, Item*> items;
	std::mutex access;
	std::function<void(void)> sync;
	
	bool done = false;
	
	double size = 200;
	
	int delay = 40; // ms
	
	const int max_plant_count = 20;
	int plant_count = 0;
	
	const int max_plant_timer = 1000;
	int plant_timer = max_plant_timer;
	
	std::minstd_rand rand_engine;
	std::uniform_real_distribution<> rand_dist;
	
	double rand() {
		return rand_dist(rand_engine);
	}
	
	vec2 rand2() {
		return vec2(rand(), rand());
	}
	
	vec2 rand_pos(int is) {
		return 2*rand2()*(size - is);
	}
	
	World() : rand_dist(-0.5, 0.5) {
		for(int i = 0; i < 1; ++i) {
			Item *item = new Animal();
			item->pos = rand_pos(item->size);
			items.insert(std::pair<int, Item*>(id_counter++, item));
		}
	}
	~World() {
		for(auto &p : items) {
			delete p.second;
		}
	}
	void operator()() {
		while(!done) {
			access.lock();
			{
				// consume
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *anim = static_cast<Animal*>(item);
						for(auto &op : items) {
							if(op.second->type == 0) {
								Plant *p = static_cast<Plant*>(op.second);
								if(p->exists && length(p->pos - anim->pos) < 0.5*(p->size + anim->size)) {
									p->exists = false;
									plant_count -= 1;
								}
							}
						}
					}
				}
				for(auto ii = items.begin(); ii != items.end();) {
					if(ii->second->exists) {
						++ii;
					} else {
						auto iv = ii->second;
						items.erase(ii++);
					}
				}
				
				// spawn
				if(plant_timer <= 0) {
					plant_timer = max_plant_timer;
					if(plant_count < max_plant_count) {
						Plant *plant = new Plant();
						plant->pos = rand_pos(plant->size);
						items.insert(std::pair<int, Item*>(id_counter++, plant));
						plant_count += 1;
					}
				}
				
				// set inputs
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *anim = static_cast<Animal*>(item);
						vec2 dir = nullvec2;
						double pot = 0.0;
						for(auto &op : items) {
							if(op.second->type == 0) {
								Plant *p = static_cast<Plant*>(op.second);
								vec2 d = anim->pos - p->pos;
								double l = length(d) + 1e-2;
								dir += -d/(l*l*l);
								pot += 1.0/l;
							}
						}
						dir = normalize(dir);
						anim->entity.input[0] = dir[0];
						anim->entity.input[1] = dir[1];
						anim->entity.input[2] = pot;
					}
				}
				
				// process
				for(auto &p : items) {
					Item *item = p.second;
					item->proc();
				}
				
				// get outputs
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *anim = static_cast<Animal*>(item);
						float *out = anim->entity.output.data();
						vec2 dir(out[0], out[1]);
						if(length(dir) > 1e-2) {
							anim->vel = 60.0*normalize(dir);
						} else {
							anim->vel = nullvec2;
						}
					}
				}
				
				// move items
				double dt = delay*1e-3;
				for(auto &p : items) {
					Item *item = p.second;
					item->pos += item->vel*dt;
				}
				
				sync();
			}
			access.unlock();
			std::this_thread::sleep_for(std::chrono::milliseconds(delay));
			
			plant_timer -= delay;
		}
	}
};
