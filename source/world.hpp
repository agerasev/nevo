#pragma once

#include <functional>
#include <thread>
#include <mutex>
#include <chrono>
#include <map>
#include <cmath>
#include <random>
#include <cassert>

#include <la/vec.hpp>
#include <la/mat.hpp>

#include "entity.hpp"

#include <cstdio>

#include "vector.hpp"

class Item {
public:
	double score = 0.0;
	
	int type;
	
	bool exists = true;
	
	double mass = 1.0;
	vec2 pos = nullvec2;
	vec2 vel = nullvec2;
	
	double size() const {
		return 0.5*sqrt(score);
	}
	
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
		no = 3,
		nh = 4;
	
	Entity entity;
	slice<float> Wih, Whh, bh, Who, bo;
	slice<float> vi, vo, vh;
	vector<float> th;
	
	Animal(Entity *esrc = nullptr) : 
		Item(1),
		entity(ni, no, ni*nh + (nh*nh + nh) + nh*no + no, nh),
		th(nh)
	{
		if(esrc != nullptr) {
			entity = *esrc;
		}
		
		float *w = entity.weight.data();
		
		Wih = slice<float>(w, ni*nh);
		w += Wih.size();
		
		Whh = slice<float>(w, nh*nh);
		w += Whh.size();
		
		bh = slice<float>(w, nh);
		w += bh.size();
		
		Who = slice<float>(w, nh*no);
		w += Who.size();
		
		bo = slice<float>(w, no);
		w += bo.size();
		
		assert(w == entity.weight.data() + entity.weight.size());
		
		vi = slice<float>(entity.input.data(), ni);
		vo = slice<float>(entity.output.data(), no);
		vh = slice<float>(entity.memory.data(), nh);
	}
	
	virtual void proc() override {
		dot(th, Wih, vi);
		
		dot(vh, Whh, vh);
		add(vh, th, vh);
		add(vh, vh, bh);
		tanh(vh, vh);
		
		dot(vo, Who, vh);
		add(vo, bo, vo);
	}
};

class World {
public:
	int id_counter = 1;
	
	std::map<int, Item*> items;
	std::mutex access;
	std::function<void(void)> sync;
	
	bool done = false;
	
	vec2 size = vec2(1280, 720)/2 - vec2(10, 10);
	
	int delay = 40; // ms
	int draw_freq = 1;
	
	const int max_plant_timer = 3;
	int plant_timer = max_plant_timer;
	
	const int max_plant_count = 500;
	int plant_count = 0;
	
	const int min_anim_count = 20;
	int anim_count = 0;
	
	const int max_breed_timer = 100;
	int breed_timer = max_breed_timer;
	
	double max_speed = 100.0;
	
	double eat_factor = 0.1;
	double time_fine = 2.0;
	double move_fine = 0.0;
	
	double breed_threshold = 400.0;
	double init_score = 100.0;
	
	double plant_max_score = 400.0;
	double plant_grow_speed = 1.0;
	
	std::minstd_rand rand_engine;
	std::uniform_real_distribution<> rand_dist;
	
	double rand() {
		return rand_dist(rand_engine);
	}
	
	vec2 rand2() {
		return vec2(rand(), rand());
	}
	
	vec2 rand_pos(double is) {
		return 2*rand2()*(size - vec2(is, is));
	}
	
	World() : rand_dist(-0.5, 0.5) {
		for(int i = 0; i < min_anim_count; ++i) {
			Animal *anim = new Animal();
			anim->pos = rand_pos(anim->size());
			anim->score = init_score;
			anim->entity.randomize([this](){return rand();});
			items.insert(std::pair<int, Item*>(id_counter++, anim));
			anim_count += 1;
		}
		for(int i = 0; i < max_plant_count; ++i) {
			Plant *plant = new Plant();
			plant->pos = rand_pos(plant->size());
			plant->score = plant_max_score;
			items.insert(std::pair<int, Item*>(id_counter++, plant));
			plant_count += 1;
		}
	}
	~World() {
		for(auto &p : items) {
			delete p.second;
		}
	}
	void operator()() {
		int counter = 0;
		while(!done) {
			access.lock();
			{
				// consume
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *anim = static_cast<Animal*>(item);
						// eat plants
						for(auto &op : items) {
							if(op.second->type == 0) {
								Plant *p = static_cast<Plant*>(op.second);
								if(p->exists && length(p->pos - anim->pos) < 0.8*(p->size() + anim->size())) {
									p->exists = false;
									plant_count -= 1;
									anim->score += p->score*eat_factor;
								}
							}
						}
						// die
						if(anim->score < 0.0) {
							anim->exists = false;
							anim_count -= 1;
						}
					}
				}
				for(auto ii = items.begin(); ii != items.end();) {
					if(ii->second->exists) {
						++ii;
					} else {
						items.erase(ii++);
					}
				}
				
				// spawn plant
				while(plant_count < max_plant_count) { // && plant_timer <= 0) {
					Plant *plant = new Plant();
					plant->pos = rand_pos(plant->size());
					items.insert(std::pair<int, Item*>(id_counter++, plant));
					plant_count += 1;
				}
				// spawn anim
				while(anim_count < min_anim_count) {
					Animal *anim = new Animal();
					anim->pos = rand_pos(anim->size());
					anim->score = init_score;
					anim->entity.randomize([this](){return rand();});
					items.insert(std::pair<int, Item*>(id_counter++, anim));
					anim_count += 1;
				}
				// breed animal
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *asrc = static_cast<Animal*>(item);
						if(asrc->score > breed_threshold) {
							Animal *anim = new Animal(&asrc->entity);
							asrc->score /= 2;
							anim->score = asrc->score;
							vec2 dir = normalize(rand2());
							anim->pos = asrc->pos + 0.5*dir*asrc->size();
							asrc->pos -= 0.5*dir*asrc->size();
							anim->entity.vary([this](){return rand();}, 0.04);
							items.insert(std::pair<int, Item*>(id_counter++, anim));
							anim_count += 1;
						}
					}
				}
				
				// update scores
				for(auto &p : items) {
					Item *item = p.second;
					if(item->type == 1) {
						Animal *anim = static_cast<Animal*>(item);
						anim->score -= time_fine + move_fine*length(anim->vel)/max_speed;
					} else if(item->type == 0) {
						Plant *p = static_cast<Plant*>(item);
						if(p->score < plant_max_score) {
							p->score += plant_grow_speed;
							if(p->score > plant_max_score) {
								p->score = plant_max_score;
							}
						}
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
								vec2 d = 0.5*(p->pos - anim->pos)/size.y();
								double l = length(d) + 1e-2;
								double m = p->score/plant_max_score;
								dir += m*d/(l*l*l);
								pot += m/l;
							}
						}
						if(plant_count > 0) {
							dir /= plant_count;
							pot /= plant_count;
						}
						
						if(length(dir) > 1e-2) {
							dir = normalize(dir);
						} else {
							dir = nullvec2;
						}
						
						//fprintf(stderr, "%f %f %f\n", dir.x(), dir.y(), pot);
						
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
						double spd = max_speed*fabs(tanh(out[2]));
						if(length(dir) > 1e-2) {
							anim->vel = spd*normalize(dir);
						} else {
							anim->vel = nullvec2;
						}
						//fprintf(stderr, "%f %f %f\n", dir.x(), dir.y(), spd);
					}
				}
				
				// move items
				double dt = 1e-1;
				for(auto &p : items) {
					Item *item = p.second;
					item->pos += item->vel*dt;
				}
				for(auto &p : items) {
					Item *item = p.second;
					vec2 msize = size - vec2(item->size(), item->size());
					if(item->pos.x() < -msize.x()) {
						item->pos.x() = -msize.x();
					} else if(item->pos.x() > msize.x()) {
						item->pos.x() = msize.x();
					}
					if(item->pos.y() < -msize.y()) {
						item->pos.y() = -msize.y();
					} else if(item->pos.y() > msize.y()) {
						item->pos.y() = msize.y();
					}
				}
				
				if(counter % draw_freq == 0)
					sync();
			}
			access.unlock();
			std::this_thread::sleep_for(std::chrono::milliseconds(delay));
			
			if(breed_timer <= 0) {
				breed_timer = max_breed_timer;
			}
			breed_timer -= 1;
			
			if(plant_timer <= 0) {
				plant_timer = max_plant_timer;
			}
			plant_timer -= 1;
			
			counter += 1;
		}
	}
};
