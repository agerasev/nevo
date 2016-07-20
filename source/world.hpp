#pragma once

#include <functional>
#include <thread>
#include <mutex>
#include <chrono>
#include <map>
#include <cmath>

#include <la/vec.hpp>
#include <la/mat.hpp>


class Item {
public:
	vec2 pos;
};

class World {
public:
	std::map<int, Item*> items;
	std::mutex access;
	std::function<void(void)> sync;
	bool done = false;
	double size = 200;
	
	World() {
		for(int i = 0; i < 4; ++i) {
			Item *item = new Item();
			item->pos = vec2(50*i, 0);
			items.insert(std::pair<int, Item*>(i + 1, item));
		}
	}
	~World() {
		for(auto &p : items) {
			delete p.second;
		}
	}
	void operator()() {
		while(!done) {
			access.lock();
			{
				double da = 0.01;
				mat2 rm(cos(da), -sin(da), sin(da), cos(da));
				for(auto &p : items) {
					Item *item = p.second;
					item->pos = rm*item->pos;
				}
				sync();
			}
			access.unlock();
			std::this_thread::sleep_for(std::chrono::milliseconds(40));
		}
	}
};
