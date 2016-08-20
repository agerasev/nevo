#pragma once

#include <cmath>
#include <list>

#include <la/vec.hpp>
#include <la/mat.hpp>

#include "vector.hpp"
#include "random.hpp"
#include "mind.hpp"

#include <core/entity.hpp>

class Organism : public Entity {
public:
	double _score = 0.0;
	
	double energy = 0.0;
	bool alive = true;
	
	long total_age = 0;
	int age = 0, anc = 0;
	
	virtual double size() const {
		return 0.5*sqrt(energy);
	}
	
	virtual double score() const {
		return _score;
	}
	
	virtual void process() {
		age += 1;
		total_age += 1;
	}
	
	virtual std::list<Organism*> produce() {
		return std::list<Organism*>();
	}
};

struct PG {
	double pot = 0.0;
	vec2 grad = nullvec2;
};

class Plant : public Organism {
public:
	static const double constexpr
		init_energy = 0.1,
		lower_energy = 300.0,
		upper_energy = 700.0,
		score_fine = 1.0,
		grow_speed = 2.0,
		grow_exp = 0.0, //0.001,
		max_age = 2000;
	
	double max_score = lower_energy;
	
	Plant() {
		max_score = lower_energy + (upper_energy - lower_energy)*rand_unif();
	}
	
	virtual void interact(Entity *e) override {}
	
	void process() override {
		Organism::process();
		// die
		if(energy <= 0.0 || age + score_fine*(energy - lower_energy) > max_age) {
			alive = false;
			return;
		}
		// grow
		if(energy < max_score) {
			energy += grow_speed + grow_exp*energy;
			if(energy > max_score) {
				energy = max_score;
			}
		}
	}
};

class Animal : public Organism {
public:
	double 
		max_speed,
		max_spin,
	
		eat_factor,
		time_fine,
		spin_fine,
	
		breed_energy,
		max_age,
	
		breed_factor,
		mind_delta;
	
	int child_count = 2;
	
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
		
		max_spin = 10.0;
		energy = 100.0;
		spin_fine = 0.1;
		breed_factor = 2.0;
		mind_delta = 0.01;
		
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
	
	double score() const override {
		return _score;
	}
	
	virtual bool edible(const Organism *e) const = 0;
	
	void interact(Entity *e) override {
		// eat
		Organism *o = static_cast<Organism*>(e);
		if(edible(o)) {
			if(o->alive && length(o->pos - pos) < 0.8*(o->size() + size())) {
				double ae = o->energy*eat_factor;
				energy += ae;
				o->energy = 0.0;
				_score += ae;
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
		Organism::process();
		
		// update scores
		energy -= time_fine + spin_fine*fabs(spin);
		
		// check able to live
		if(energy < 0.0 || age > max_age) {
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
	
	std::list<Organism*> produce() override {
		std::list<Organism*> list;
		
		if(energy > breed_energy) {
			for(int i = 0; i < child_count; ++i) {
				Animal *anim = instance();
				
				anim->energy = energy/child_count;
				anim->total_age = total_age;
				anim->anc = (anc += 1);
				
				anim->pos = pos + 0.5*rand_disk()*size();
				
				anim->mind.vary(rand_norm, mind_delta);
				
				list.push_back(anim);
			}
			
			_score += breed_factor*(max_age - age);
			alive = false;
		}
		
		return list;
	}
	
	void move(double dt) override {
		Organism::move(dt);
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
		
		max_age = 500;
		
		eat_factor = 0.2;
		time_fine = 1.0;
	
		breed_energy = 800.0;
	}
	
	bool edible(const Organism *e) const override {
		return dynamic_cast<const Plant*>(e) != nullptr;
	}
	
	Herbivore *instance() const override {
		return new Herbivore(&mind);
	}
};

class Carnivore : public Animal {
public:
	double eat_energy = 0.2;
	
	Carnivore(const Mind *ms = nullptr) : Animal(ms) {
		max_speed = 100.0;
		
		max_age = 1000;
		
		eat_factor = 0.2;
		time_fine = 0.5;
	
		breed_energy = 1000.0;
	}
	
	bool edible(const Organism *e) const override {
		auto h = dynamic_cast<const Herbivore*>(e);
		if(h == nullptr)
			return false;
		if(eat_energy*h->energy > energy)
			return false;
		return true;
	}
	
	Carnivore *instance() const override {
		return new Carnivore(&mind);
	}
};
