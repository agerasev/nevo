#include <QtGui/QApplication>
#include <QtGui/QWidget>
//#include <QtGui/QGraphicsView>

class MainWindow : public QWidget {
public:
	//QGraphicsView view;
	//QGraphicsScene scene;
	
	MainWindow() : QWidget() {
		
		
		resize(800, 600);
		setWindowTitle("NEvo");
	}
};

int main(int argc, char *argv[]) {
	
	QApplication app(argc, argv);
	
	MainWindow window;
	window.show();
	
	return app.exec();
}
