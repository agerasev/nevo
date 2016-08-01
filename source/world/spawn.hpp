#pragma once

#include "entity.hpp"

class Spawn : public Entity {
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
	}
	
	double size() const override {
		return rad;
	}
	
	vec2 rand_pos() const {
		return pos + rad*rand_disk();
	}
	
	virtual Entity *instance() const = 0;
	virtual bool own(const Entity *e) const = 0;
	
	void interact(Entity *e) override {
		if(own(e) && length(pos - e->pos) < rad) {
			count += 1;
		}
	}
	
	virtual void process() override {
		Entity::process();
		timer += 1;
	}
	
	virtual std::list<Entity*> produce() override {
		std::list<Entity*> list;
		
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
	
	Entity *instance() const override {
		Plant *a = new Plant();
		
		a->pos = rand_pos();
		a->score = Plant::init_score;
		
		return a;
	}
	
	bool own(const Entity *e) const override {
		return dynamic_cast<const Plant*>(e) != nullptr;
	}
};

class SpawnAnimal : public Spawn {
public:
	template <typename ... Args>
	SpawnAnimal(Args ... args) : Spawn(args...) {}
	
	Entity *instance() const override {
		Animal *a = new Animal();
		
		a->pos = rand_pos();
		a->score = Animal::init_score;
		a->mind.randomize(rand_norm);
		
		return a;
	}
	
	bool own(const Entity *e) const override {
		return dynamic_cast<const Animal*>(e) != nullptr;
	}
};

