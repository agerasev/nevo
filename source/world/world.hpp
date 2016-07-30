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
	
	const int max_plant_count = 400;
	int plant_count = 0;
	
	const int min_anim_count = 10;
	int anim_count = 0;
	
	double max_speed = 50.0;
	double max_spin = 10.0;
	
	double eat_factor = 0.1;
	double time_fine = 0.2;
	double move_fine = 0.2;
	double spin_fine = 0.1;
	
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
	std::uniform_real_distribution<> unif_dist;
	std::normal_distribution<> norm_dist;
	
	double rand() {
		return unif_dist(rand_engine);
	}
	
	double randn() {
		return norm_dist(rand_engine);
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
	
	World(const vec2 &s) : unif_dist(-0.5, 0.5) {
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
	
	std::pair<double, vec2> potential(Entity *e, std::function<bool(Entity*)> selector) {
		double pot = 0.0;
		vec2 grad = nullvec2;
		for(auto &op : entities) {
			Entity *p = op.second;
			double spot = 4*e->size();
			if(selector(p) && length(p->pos - e->pos) - p->size() < spot) {
				vec2 d = p->pos - e->pos;
				double l = length(d) + p->size();
				double m = p->size()/e->size();
				pot += m/l;
				grad += m*d/(l*l*l);
			}
		}
		double lg = length(grad);
		if(lg < 1e-4)
			grad = nullvec2;
		else
			grad = normalize(grad);
		return std::pair<double, vec2>(pot, grad);
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
					Animal *anim = dynamic_cast<Animal*>(p.second);
					if(anim == nullptr)
						continue;
					
					// eat plants
					for(auto &op : entities) {
						Plant *p = dynamic_cast<Plant*>(op.second);
						if(p == nullptr)
							continue;
						if(p->alive && length(p->pos - anim->pos) < 0.8*(p->size() + anim->size())) {
							p->alive = false;
							plant_count -= 1;
							anim->score += p->score*eat_factor;
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
					anim->mind.randomize([this](){return randn();});
					entities.insert(std::pair<int, Entity*>(id_counter++, anim));
					anim_count += 1;
				}
				// breed animal
				for(auto &p : entities) {
					Animal *asrc = dynamic_cast<Animal*>(p.second);
					if(asrc == nullptr)
						continue;
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
						
						anim->mind.vary([this](){return randn();}, 0.01);
						
						entities.insert(std::pair<int, Entity*>(id_counter++, anim));
						anim_count += 1;
					}
				}
				
				// update scores
				anim_max_age = 0;
				anim_max_anc = 0;
				for(auto &p : entities) {
					Entity *entity = p.second;
					if(Animal *anim = dynamic_cast<Animal*>(entity)) {
						anim->score -= time_fine + move_fine*length(anim->vel)/max_speed + spin_fine*fabs(anim->spin)/max_spin;
						if(anim_max_age < anim->total_age)
							anim_max_age = anim->total_age;
						if(anim_max_anc < anim->nanc)
							anim_max_anc = anim->nanc;
					} else if(Plant *p = dynamic_cast<Plant*>(entity)) {
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
					Animal *anim = dynamic_cast<Animal*>(p.second);
					if(anim == nullptr)
						continue;
					
					std::pair<double, vec2> pg;
					double pot = 0.0;
					vec2 dir = nullvec2;
					
					mat2 rot(anim->dir.x(), anim->dir.y(), -anim->dir.y(), anim->dir.x());
					
					pg = potential(anim, [](Entity *e) {return dynamic_cast<Plant*>(e) != nullptr;});
					pot = pg.first;
					dir = rot*pg.second;
					
					anim->mind.input[0] = dir[0];
					anim->mind.input[1] = dir[1];
					anim->mind.input[2] = pot;
					
					
					pg = potential(anim, [anim](Entity *e) {return dynamic_cast<Animal*>(e) != nullptr && e != anim;});
					pot = pg.first;
					dir = rot*pg.second;
					
					anim->mind.input[3] = dir[0];
					anim->mind.input[4] = dir[1];
					anim->mind.input[5] = pot;
				}
				
				// process
				for(auto &p : entities) {
					Entity *entity = p.second;
					entity->proc();
				}
				
				// get outputs
				for(auto &p : entities) {
					Animal *anim = dynamic_cast<Animal*>(p.second);
					if(anim == nullptr)
						continue;
					float *out = anim->mind.output.data();
					anim->spin = max_spin*tanh(out[1]);
					anim->vel = max_speed*fabs(tanh(out[0]))*anim->dir;
				}
				
				// move entities
				double dt = 1e-1;
				for(auto &p : entities) {
					Entity *entity = p.second;
					double da = dt*entity->spin;
					double sda = sin(da), cda = cos(da);
					mat2 rot(cda, sda, -sda, cda);
					entity->dir = normalize(rot*entity->dir);
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
