#include <QApplication>

#include <la/vec.hpp>

#include <event.hpp>
#include <graphics/graphics.hpp>
#include <world/world.hpp>


int main(int argc, char *argv[]) {
	World world(500*vec2(1, 1));
	
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
