#include <QApplication>

#include <la/vec.hpp>

#include <event.hpp>
#include <graphics/graphics.hpp>
#include <world/world.hpp>

#include "world/random.hpp"


int main(int argc, char *argv[]) {
	World world(vec2(1000, 1500));
	
	world.add(new SpawnAnimal(vec2(0, 1000), 100,  10, 0));
	world.add(new SpawnPlant( vec2(0,  800), 300,   0, 100));
	world.add(new SpawnPlant( vec2(0, -500), 1000,  0, 100));
	
	QApplication app(argc, argv);
	
	MainWindow window(&world);
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
