#pragma once

#include <cmath>

#include <la/vec.hpp>

#include "vector.hpp"

#include "mind.hpp"


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
	
	vec2 dir = vec2(1, 0);
	double spin = 0.0;
	
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
	const int neyes = 12;
	const int 
		ni = 6,
		no = 2,
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
