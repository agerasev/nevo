#pragma once

#include <view/scene.hpp>

#include "item.hpp"


class MyScene : public Scene {
public:
	MyScene(World *w) : Scene(w) {}
	
	Item *instance(Entity *e) const {
		if(auto p = dynamic_cast<Plant*>(e))
			return new ItemPlant(p);
		if(auto a = dynamic_cast<Animal*>(e))
			return new ItemAnimal(a);
		if(auto s = dynamic_cast<Spawn*>(e))
			return new ItemSpawn(s);
		return new Item(e);
	}
};
