#include <world.hpp>
#include <event.hpp>

#include <QApplication>

#include <la/vec.hpp>

#include <graphics/graphics.hpp>


int main(int argc, char *argv[]) {
	World world(vec2(640, 640)/2);
	
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
