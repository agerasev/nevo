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

#include "mind.hpp"

#include <cstdio>

#include "vector.hpp"

class Entity {
public:
	double score = 0.0;
	
	long total_age = 0, age = 0;
	int nanc = 0;
	
	int type;
	
	bool alive = true;
	
	double mass = 1.0;
	vec2 pos = nullvec2;
	vec2 vel = nullvec2;
	
	double size() const {
		return 0.5*sqrt(score);
	}
	
	Entity(int t) {
		type = t;
	}
	virtual ~Entity() = default;
	
	virtual void proc() = 0;
};

class Plant : public Entity {
public:
	Plant() : Entity(0) {
		
	}
	
	virtual void proc() override {
		
	}
};


class Animal : public Entity {
public:
	const int 
		ni = 6,
		no = 3,
		nh = 8;
	
	Mind mind;
	slice<float> Wih, Whh, bh, Who, bo;
	slice<float> vi, vo, vh;
	vector<float> th;
	
	Animal(Mind *esrc = nullptr) : 
		Entity(1),
		mind(ni, no, ni*nh + (nh*nh + nh) + nh*no + no, nh),
		th(nh)
	{
		if(esrc != nullptr) {
			mind = *esrc;
		}
		
		float *w = mind.weight.data();
		
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
		
		assert(w == mind.weight.data() + mind.weight.size());
		
		vi = slice<float>(mind.input.data(), ni);
		vo = slice<float>(mind.output.data(), no);
		vh = slice<float>(mind.memory.data(), nh);
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
	long id_counter = 1;
	
	std::map<long, Entity*> entities;
	std::mutex access;
	std::function<void(void)> sync;
	
	bool done = false;
	bool paused = false;
	
	vec2 size;
	
	int delay = 40000; // ms
	double step_duration = 0.0; // ms
	long steps_elapsed = 0;
	
	// config
	
	const int max_plant_count = 200;
	int plant_count = 0;
	
	const int min_anim_count = 20;
	int anim_count = 0;
	
	double max_speed = 50.0;
	
	double eat_factor = 0.1;
	double time_fine = 0.2;
	double move_fine = 0.2;
	
	double breed_threshold = 1000.0;
	double breed_age = 400;
	double breed_min_score = 400.0;
	double init_score = 100.0;
	
	double plant_max_score = 500.0;
	double plant_grow_speed = 1.0;
	double plant_grow_exp = 0.0; //0.001;
	
	// statistics
	
	long anim_max_age = 0;
	int anim_max_anc = 0;
	
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
	
	void setDelay(int ms) {
		delay = ms;
	}
	
	World(const vec2 &s) : rand_dist(-0.5, 0.5) {
		size = s;
		
		for(int i = 0; i < min_anim_count; ++i) {
			Animal *anim = new Animal();
			anim->pos = rand_pos(anim->size());
			anim->score = init_score;
			anim->mind.randomize([this](){return rand();});
			entities.insert(std::pair<int, Entity*>(id_counter++, anim));
			anim_count += 1;
		}
		for(int i = 0; i < max_plant_count; ++i) {
			Plant *plant = new Plant();
			plant->pos = rand_pos(plant->size());
			plant->score = plant_max_score;
			entities.insert(std::pair<int, Entity*>(id_counter++, plant));
			plant_count += 1;
		}
	}
	~World() {
		for(auto &p : entities) {
			delete p.second;
		}
	}
	
	std::pair<double, vec2> potential(const vec2 &pos, std::function<bool(Entity*)> selector) {
		vec2 dir = nullvec2;
		double pot = 0.0;
		for(auto &op : entities) {
			Entity *p = static_cast<Plant*>(op.second);
			if(selector(p)) {
				vec2 d = 0.5*(p->pos - pos)/size.y();
				double l = length(d) + 1e-2;
				double m = p->score;
				dir += m*d/(l*l*l);
				pot += m/l;
			}
		}
		return std::pair<double, vec2>(pot, dir);
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
				// consume
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(entity->type == 1) {
						Animal *anim = static_cast<Animal*>(entity);
						// eat plants
						for(auto &op : entities) {
							if(op.second->type == 0) {
								Plant *p = static_cast<Plant*>(op.second);
								if(p->alive && length(p->pos - anim->pos) < 0.8*(p->size() + anim->size())) {
									p->alive = false;
									plant_count -= 1;
									anim->score += p->score*eat_factor;
								}
							}
						}
						// die
						if(
							anim->score < 0.0 ||
							(anim->age > breed_age && anim->score < breed_min_score)
						) {
							anim->alive = false;
							anim_count -= 1;
						}
					}
				}
				for(auto ii = entities.begin(); ii != entities.end();) {
					if(ii->second->alive) {
						++ii;
					} else {
						entities.erase(ii++);
					}
				}
				
				// spawn plant
				while(plant_count < max_plant_count) { // && plant_timer <= 0) {
					Plant *plant = new Plant();
					plant->pos = rand_pos(plant->size());
					entities.insert(std::pair<int, Entity*>(id_counter++, plant));
					plant_count += 1;
				}
				// spawn anim
				while(anim_count < min_anim_count) {
					Animal *anim = new Animal();
					anim->pos = rand_pos(anim->size());
					anim->score = init_score;
					anim->mind.randomize([this](){return rand();});
					entities.insert(std::pair<int, Entity*>(id_counter++, anim));
					anim_count += 1;
				}
				// breed animal
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(entity->type == 1) {
						Animal *asrc = static_cast<Animal*>(entity);
						if((asrc->score - breed_min_score)/(breed_threshold - breed_min_score) + asrc->age/breed_age > 1.0) {
						// if(asrc->score > breed_threshold || asrc->age > breed_age) {
							Animal *anim = new Animal(&asrc->mind);
							
							asrc->score /= 2;
							anim->score = asrc->score;
							asrc->age = 0;
							anim->total_age = asrc->total_age;
							anim->nanc = (asrc->nanc += 1);
							
							vec2 dir = normalize(rand2());
							anim->pos = asrc->pos + 0.5*dir*asrc->size();
							asrc->pos -= 0.5*dir*asrc->size();
							
							anim->mind.vary([this](){return rand();}, 0.04);
							
							entities.insert(std::pair<int, Entity*>(id_counter++, anim));
							anim_count += 1;
						}
					}
				}
				
				// update scores
				anim_max_age = 0;
				anim_max_anc = 0;
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(entity->type == 1) {
						Animal *anim = static_cast<Animal*>(entity);
						anim->score -= time_fine + move_fine*length(anim->vel)/max_speed;
						if(anim_max_age < anim->total_age)
							anim_max_age = anim->total_age;
						if(anim_max_anc < anim->nanc)
							anim_max_anc = anim->nanc;
					} else if(entity->type == 0) {
						Plant *p = static_cast<Plant*>(entity);
						if(p->score < plant_max_score) {
							p->score += plant_grow_speed + plant_grow_exp*p->score;
							if(p->score > plant_max_score) {
								p->score = plant_max_score;
							}
						}
					}
					entity->age += 1;
					entity->total_age += 1;
				}
				
				// set inputs
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(entity->type == 1) {
						Animal *anim = static_cast<Animal*>(entity);
						
						std::pair<double, vec2> pg;
						double pot = 0.0;
						vec2 dir = nullvec2;
						
						pg = potential(anim->pos, [](Entity *e) {return e->type == 0;});
						pot = pg.first;
						dir = pg.second;
						if(plant_count > 0) {
							pot /= plant_count*plant_max_score;
							dir /= plant_count*plant_max_score;
						}
						if(length(dir) > 1e-2) {
							dir = normalize(dir);
						} else {
							dir = nullvec2;
						}
						
						anim->mind.input[0] = dir[0];
						anim->mind.input[1] = dir[1];
						anim->mind.input[2] = pot;
						
						pg = potential(anim->pos, [anim](Entity *e) {return e->type == 1 && e != anim;});
						pot = pg.first;
						dir = pg.second;
						if(anim_count > 0) {
							pot /= anim_count*breed_threshold;
							dir /= anim_count*breed_threshold;
						}
						if(length(dir) > 1e-2) {
							dir = normalize(dir);
						} else {
							dir = nullvec2;
						}
						
						anim->mind.input[3] = dir[0];
						anim->mind.input[4] = dir[1];
						anim->mind.input[5] = pot;
					}
				}
				
				// process
				for(auto &p : entities) {
					Entity *entity = p.second;
					entity->proc();
				}
				
				// get outputs
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(entity->type == 1) {
						Animal *anim = static_cast<Animal*>(entity);
						float *out = anim->mind.output.data();
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
				
				// move entities
				double dt = 1e-1;
				for(auto &p : entities) {
					Entity *entity = p.second;
					entity->pos += entity->vel*dt;
				}
				for(auto &p : entities) {
					Entity *entity = p.second;
					vec2 msize = size - vec2(entity->size(), entity->size());
					if(entity->pos.x() < -msize.x()) {
						entity->pos.x() = -msize.x();
					} else if(entity->pos.x() > msize.x()) {
						entity->pos.x() = msize.x();
					}
					if(entity->pos.y() < -msize.y()) {
						entity->pos.y() = -msize.y();
					} else if(entity->pos.y() > msize.y()) {
						entity->pos.y() = msize.y();
					}
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
