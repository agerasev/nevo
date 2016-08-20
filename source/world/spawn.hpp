#pragma once

#include "organism.hpp"

class Spawn : public Organism {
public:
	double rad = 0.0;
	double timer = 0.0, max_time;
	bool instant = false;
	int count = 0, max_count;
	
	Spawn(const vec2 &p, double r, double t, int n) {
		pos = p;
		rad = r;
		max_time = t;
		max_count = n;
		if(max_time < 1e-8)
			instant = true;
		active = true;
		interactive = false;
	}
	
	double size() const override {
		return rad;
	}
	
	vec2 rand_pos() const {
		return pos + rad*rand_disk();
	}
	
	virtual Organism *instance() const = 0;
	virtual bool own(const Organism *e) const = 0;
	
	void interact(Entity *e) override {
		Organism *o = static_cast<Organism*>(e);
		if(max_count > 0 && own(o) && length(pos - o->pos) < rad) {
			count += 1;
		}
	}
	
	virtual void process() override {
		Organism::process();
		timer += 1;
	}
	
	virtual std::list<Organism*> produce() override {
		std::list<Organism*> list;
		
		while((count < max_count || max_count == 0) && (timer >= max_time || instant)) {
			timer -= max_time;
			count += 1;
			list.push_back(instance());
		}
		count = 0;
		
		return list;
	}
	
	virtual void move(double dt) override {}
};

class SpawnPlant : public Spawn {
public:
	template <typename ... Args>
	SpawnPlant(Args ... args) : Spawn(args...) {}
	
	Plant *instance() const override {
		Plant *a = new Plant();
		
		a->pos = rand_pos();
		a->energy = Plant::init_energy;
		
		return a;
	}
	
	bool own(const Organism *e) const override {
		return dynamic_cast<const Plant*>(e) != nullptr;
	}
};

class SpawnAnimal : public Spawn {
public:
	std::function<const Mind*()> mindgen = [](){return nullptr;};
	
	template <typename ... Args>
	SpawnAnimal(Args ... args) : Spawn(args...) {}
	
	bool own(const Organism *e) const override {
		return dynamic_cast<const Animal*>(e) != nullptr;
	}
};

class SpawnHerbivore : public SpawnAnimal {
public:
	template <typename ... Args>
	SpawnHerbivore(Args ... args) : SpawnAnimal(args...) {}
	
	Herbivore *instance() const override {
		const Mind *m = mindgen();
		Herbivore *a = new Herbivore(m);
		
		if(m != nullptr) {
			a->mind.vary(rand_norm, a->mind_delta);
		} else {
			a->mind.randomize(rand_norm);
		}
		
		a->pos = rand_pos();
		
		return a;
	}
	
	bool own(const Organism *e) const override {
		return dynamic_cast<const Herbivore*>(e) != nullptr;
	}
};

class SpawnCarnivore : public SpawnAnimal {
public:
	template <typename ... Args>
	SpawnCarnivore(Args ... args) : SpawnAnimal(args...) {}
	
	Carnivore *instance() const override {
		const Mind *m = mindgen();
		Carnivore *a = new Carnivore(m);
		
		if(m != nullptr) {
			a->mind.vary(rand_norm, a->mind_delta);
		} else {
			a->mind.randomize(rand_norm);
		}
		
		a->pos = rand_pos();
		
		return a;
	}
	
	bool own(const Organism *e) const override {
		return dynamic_cast<const Carnivore*>(e) != nullptr;
	}
};
