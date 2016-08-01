#pragma once

#include <cmath>
#include <list>
#include <cstdio>

#include <la/vec.hpp>
#include <la/mat.hpp>

#include "vector.hpp"
#include "random.hpp"
#include "mind.hpp"


class Entity {
public:
	double score = 0.0;
	bool alive = true;
	bool active = false;
	
	double mass = 1.0;
	vec2 pos = nullvec2;
	vec2 vel = nullvec2;
	
	long total_age = 0;
	int age = 0, anc = 0;
	
	virtual double size() const {
		return 0.5*sqrt(score);
	}
	
	Entity() = default;
	virtual ~Entity() = default;
	
	virtual void interact(Entity *e) = 0;
	
	virtual void process() {
		age += 1;
		total_age += 1;
	}
	
	virtual std::list<Entity*> produce() = 0;
	
	virtual void move(double dt) {
		pos += vel*dt;
	}
};


class Plant : public Entity {
public:
	static const double constexpr
		init_score = 0.1,
		lower_score = 300.0,
		upper_score = 700.0,
		score_fine = 1.0,
		grow_speed = 2.0,
		grow_exp = 0.0, //0.001,
		max_age = 2000;
	
	double max_score = lower_score;
	
	Plant() {
		max_score = lower_score + (upper_score - lower_score)*rand_unif();
	}
	
	virtual void interact(Entity *e) override {}
	
	void process() override {
		Entity::process();
		// die
		if(score <= 0.0 || age + score_fine*(score - lower_score) > max_age) {
			alive = false;
			return;
		}
		// grow
		if(score < max_score) {
			score += grow_speed + grow_exp*score;
			if(score > max_score) {
				score = max_score;
			}
		}
	}
	
	std::list<Entity*> produce() override {
		return std::list<Entity*>();
	}
};

struct PG {
	double pot = 0.0;
	vec2 grad = nullvec2;
};

class Animal : public Entity {
public:
	double 
		max_speed,
		max_spin,
	
		eat_factor,
		time_fine,
	
		breed_score,
		max_age;
	
	vec2 dir = vec2(1, 0);
	double spin = 0.0;
	
	// const int neyes = 12;
	const int 
		ni = 3*3,
		no = 2,
		nh = 16;
	
	Mind mind;
	slice<float> Wih, Whh, bh, Who, bo;
	slice<float> vi, vo, vh;
	vector<float> th;
	
	Animal(const Mind *esrc = nullptr) : 
		mind(ni, no, ni*nh + (nh*nh + nh) + nh*no + no, nh),
		th(nh)
	{
		active = true;
		
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
		
		vi = slice<float>(mind.input.data(), ni);
		vo = slice<float>(mind.output.data(), no);
		vh = slice<float>(mind.memory.data(), nh);
	}
	
	virtual bool edible(const Entity *e) const = 0;
	
	void interact(Entity *e) override {
		// eat
		if(edible(e)) {
			if(e->alive && length(e->pos - pos) < 0.8*(e->size() + size())) {
				score += e->score*eat_factor;
				e->score = 0.0;
			}
		}
	}
	
	virtual void sense(const std::vector<PG> &pl) {
		mat2 rot(dir.x(), dir.y(), -dir.y(), dir.x());
		
		for(int i = 0; i < int(pl.size()); ++i) {
			double p = pl[i].pot;
			vec2 d = rot*pl[i].grad;
			
			mind.input[3*i + 0] = d[0];
			mind.input[3*i + 1] = d[1];
			mind.input[3*i + 2] = p;
		}
	}
	
	void process() override {
		Entity::process();
		
		// update scores
		score -= time_fine;
		
		// check able to live
		if(score < 0.0 || age > max_age) {
			// die
			alive = false;
			return;
		}
		
		// mind step
		dot(th, Wih, vi);
		dot(vh, Whh, vh);
		add(vh, th, vh);
		add(vh, vh, bh);
		tanh(vh, vh);
		dot(vo, Who, vh);
		add(vo, bo, vo);
		
		// get outputs
		float *out = mind.output.data();
		vel = max_speed*fabs(tanh(out[0]))*dir;
		spin = max_spin*tanh(out[1]);
	}
	
	virtual Animal *instance() const = 0;
	
	std::list<Entity*> produce() override {
		std::list<Entity*> list;
		
		if(score > breed_score) {
			Animal *anim = instance();
			
			score /= 2;
			anim->score = score;
			age = 0;
			anim->total_age = total_age;
			anim->anc = (anc += 1);
			
			vec2 dir = normalize(rand_unif2() - vec2(0.5, 0.5));
			anim->pos = pos + 0.5*dir*size();
			pos -= 0.5*dir*size();
			
			anim->mind.vary(rand_norm, 0.01);
			
			list.push_back(anim);
		}
		
		return list;
	}
	
	void move(double dt) override {
		Entity::move(dt);
		double da = dt*spin;
		double sda = sin(da), cda = cos(da);
		mat2 rot(cda, sda, -sda, cda);
		dir = normalize(rot*dir);
	}
};

class Herbivore : public Animal {
public:
	Herbivore(const Mind *ms = nullptr) : Animal(ms) {
		max_speed = 100.0;
		max_spin = 10.0;
	
		eat_factor = 0.2;
		time_fine = 1.0;
	
		breed_score = 800.0;
		max_age = 500;
		
		score = 100.0;
	}
	
	bool edible(const Entity *e) const override {
		return dynamic_cast<const Plant*>(e) != nullptr;
	}
	
	Herbivore *instance() const override {
		return new Herbivore(&mind);
	}
};

class Carnivore : public Animal {
public:
	Carnivore(const Mind *ms = nullptr) : Animal(ms) {
		max_speed = 200.0;
		max_spin = 10.0;
	
		eat_factor = 0.3;
		time_fine = 0.4;
	
		breed_score = 1000.0;
		max_age = 500;
		
		score = 100.0;
	}
	
	bool edible(const Entity *e) const override {
		return dynamic_cast<const Herbivore*>(e) != nullptr;
	}
	
	Carnivore *instance() const override {
		return new Carnivore(&mind);
	}
};
