#include <QApplication>

#include <la/vec.hpp>

#include <event.hpp>

#include <graphics/window.hpp>
#include <world/myworld.hpp>

#include "world/random.hpp"


int main(int argc, char *argv[]) {
	MyWorld world(vec2(1000, 1600));
	
	world.add(new SpawnHerbivore(vec2(0, 1500), 100, 10, 0));
	world.add(new SpawnPlant(vec2(0, 1300), 300, 0, 100));
	
	world.add(new SpawnPlant(vec2(0, 0), 1000, 0, 200));
	
	world.add(new SpawnCarnivore(vec2(0, -1100), 100, 100, 0));
	world.add(new SpawnHerbivore(vec2(0, -1500), 100, 10, 0));
	world.add(new SpawnPlant(vec2(0, -1300), 300, 0, 100));
	
	QApplication app(argc, argv);
	
	Window window(&world);
	window.show();
	
	
	world.sync = [&app, &window]() {
		app.postEvent(&window, new SyncEvent());
	};
	std::thread thread([&world](){world();});

	int rs = app.exec();

	world.done = true;
	thread.join();

	return rs;
}
