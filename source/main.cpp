#include <QtGui/QApplication>
#include <QtGui/QWidget>
#include <QtGui/QGraphicsView>
#include <QtGui/QHBoxLayout>
#include <QtGui/QWheelEvent>
#include <QtGui/QGraphicsEffect>
#include <QtSvg/QGraphicsSvgItem>

#include <thread>
#include <map>
#include <cstdio>
#include <cmath>

class Item {
public:
	int id;
	Item(int _id = 0) {
		id = _id;
	}
};

class ItemEvent : public QEvent {
public:
	enum Action {
		ADD,
		DEL
	};

	Action act;
	Item *item;

	ItemEvent(Action a, Item *it) : QEvent(QEvent::User) {
		item = it;
		act = a;
	}
};

class MainScene : public QGraphicsScene {
public:
	std::map<int, QGraphicsItem*> items;

	MainScene() : QGraphicsScene() {

	}

	virtual ~MainScene() {
		for(auto &p : items) {
			delete p.second;
		}
	}

	virtual bool event(QEvent *event) override {
		if(event->type() == QEvent::User) {
			ItemEvent *ie = static_cast<ItemEvent*>(event);
			int id = ie->item->id;
			if(ie->act == ItemEvent::ADD) {
				auto gi = new QGraphicsEllipseItem(id*20, 0, 10, 10);
				addItem(gi);
				items.insert(std::pair<int, QGraphicsItem*>(id, gi));
			} else if(ie->act == ItemEvent::DEL) {
				auto ii = items.find(id);
				if(ii != items.end()) {
					auto gi = (*ii).second;
					removeItem(gi);
					items.erase(ii);
				}
			} else {
				return false;
			}
			return true;
		} else {
			return QGraphicsScene::event(event);
		}
	}
};

class MainView : public QGraphicsView {
public:
	float zf = 1.25;

	MainView() : QGraphicsView() {
		setStyleSheet( "border-style: none;");

		//setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
		//setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

		setDragMode(ScrollHandDrag);
	}

	virtual void wheelEvent(QWheelEvent *event) override {
		float z = pow(zf, event->delta()/120.0);
		scale(z,z);
	}
};

class MainWindow : public QWidget {
public:
	MainScene scene;
	MainView view;

	QWidget panel;

	QHBoxLayout layout;
	
	MainWindow() : QWidget() {
		view.setScene(&scene);

		layout.addWidget(&view, 2);
		layout.addWidget(&panel, 1);

		setLayout(&layout);
		
		resize(800, 600);
		setWindowTitle("NEvo");
	}
};

class ThreadFunc {
public:
	bool done = false;
	QApplication *app;
	QObject *dst;
	Item item;
	ThreadFunc(QApplication *_app, QObject *_dst) {
		app = _app;
		dst = _dst;
	}
	void operator ()() {
		app->postEvent(dst, new ItemEvent(ItemEvent::ADD, &item));
	}
};

int main(int argc, char *argv[]) {
	
	QApplication app(argc, argv);
	
	MainWindow window;
	window.show();
	
	ThreadFunc tf(&app, &window.scene);
	std::thread thread(tf);

	int rs = app.exec();

	tf.done = true;
	thread.join();

	return rs;
}
