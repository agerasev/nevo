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
	
	double size() const {
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
		max_score = 500.0,
		grow_speed = 1.0,
		grow_exp = 0.0; //0.001;
	
	virtual void interact(Entity *e) override {}
	
	void process() override {
		Entity::process();
		// die
		if(score <= 0.0) {
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


class Animal : public Entity {
public:
	static const double constexpr
		max_speed = 50.0,
		max_spin = 10.0,
	
		eat_factor = 0.1,
		time_fine = 0.2,
		move_fine = 0.2,
		spin_fine = 0.1,
	
		breed_threshold = 1000.0,
		breed_age = 400,
		breed_min_score = 400.0,
		init_score = 100.0;
	
	vec2 dir = vec2(1, 0);
	double spin = 0.0;
	
	// const int neyes = 12;
	const int 
		ni = 6,
		no = 2,
		nh = 8;
	
	Mind mind;
	slice<float> Wih, Whh, bh, Who, bo;
	slice<float> vi, vo, vh;
	vector<float> th;
	
	Animal(Mind *esrc = nullptr) : 
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
	
	void interact(Entity *e) override {
		// eat plants
		Plant *p = dynamic_cast<Plant*>(e);
		if(p == nullptr)
			return;
		if(p->alive && length(p->pos - pos) < 0.8*(p->size() + size())) {
			score += p->score*eat_factor;
			p->score = 0.0;
		}
	}
	
	virtual void sense(const std::pair<double, vec2> &pp, const std::pair<double, vec2> &ap) {
		double p = 0.0;
		vec2 d = nullvec2;
		
		mat2 rot(dir.x(), dir.y(), -dir.y(), dir.x());
		
		p = pp.first;
		d = rot*pp.second;
		
		mind.input[0] = d[0];
		mind.input[1] = d[1];
		mind.input[2] = p;
		
		p = ap.first;
		d = rot*ap.second;
		
		mind.input[3] = d[0];
		mind.input[4] = d[1];
		mind.input[5] = p;
	}
	
	void process() override {
		Entity::process();
		
		// update scores
		score -= time_fine + move_fine*length(vel)/max_speed + spin_fine*fabs(spin)/max_spin;
		
		// check able to live
		if(score < 0.0 || (age > breed_age && score < breed_min_score)) {
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
	
	std::list<Entity*> produce() override {
		std::list<Entity*> list;
		
		if((score - breed_min_score)/(breed_threshold - breed_min_score) + age/breed_age > 1.0) {
		// if(score > breed_threshold || age > breed_age) {
			Animal *anim = new Animal(&mind);
			
			score /= 2;
			anim->score = score;
			age = 0;
			anim->total_age = total_age;
			anim->anc = (anc += 1);
			
			vec2 dir = normalize(randu2() - vec2(0.5, 0.5));
			anim->pos = pos + 0.5*dir*size();
			pos -= 0.5*dir*size();
			
			anim->mind.vary([this](){return randn();}, 0.01);
			
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
