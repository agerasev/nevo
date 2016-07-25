#include <world.hpp>
#include <event.hpp>

#include <QtGui/QApplication>
#include <QtGui/QWidget>
#include <QtGui/QGraphicsItem>
#include <QtGui/QGraphicsView>
#include <QtGui/QHBoxLayout>
#include <QtGui/QLabel>
#include <QtGui/QPushButton>

#include <map>
#include <cmath>
#include <string>

QPointF v2q(const vec2 &v) {
	return QPointF(v.x(), v.y());
}

QColor qmix(const QColor &a, const QColor &b, double r = 0.5) {
	return QColor(
		a.red()*r + b.red()*(1 - r),
		a.green()*r + b.green()*(1 - r),
		a.blue()*r + b.blue()*(1 - r),
		255
	);
}

class ItemView : public QGraphicsItem {
public:
	bool exists = true;
	double size = 20.0;
	
	QColor color;
	
	ItemView() : QGraphicsItem() {
		
	}
	
	virtual QRectF boundingRect() const {
		return QRectF(-size, -size, 2*size, 2*size);
	}
	
	virtual void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget = 0) {
		QPen pen;
		pen.setWidth(2);
		pen.setCosmetic(true);
		pen.setColor(qmix(color, QColor("#000000"), 0.75));
		painter->setPen(pen);
		
		painter->setBrush(color);
		painter->drawEllipse(-size, -size, 2*size, 2*size);
	}
};

class MainScene : public QGraphicsScene {
public:
	std::map<int, ItemView*> items;
	World *world = nullptr;

	MainScene(World *w) : QGraphicsScene() {
		world = w;
		
		setBackgroundBrush(QColor("#FFFFFF"));
		setSceneRect(-world->size, -world->size, 2*world->size, 2*world->size);
		addRect(sceneRect(), QPen(), QBrush(QColor("#FFFFCC")));
	}

	virtual ~MainScene() {
		for(auto &p : items) {
			delete p.second;
		}
	}
	
	void sync_item(Item *raw_item, ItemView *item) {
		item->setPos(v2q(raw_item->pos));
	}
	
	ItemView *instance(Item *raw_item) {
		int t = raw_item->type;
		ItemView *item = new ItemView();
		item->size = raw_item->size;
		if(t == 0) {
			item->color = QColor("#22CC22");
		} else if(t == 1) {
			item->color = QColor("#FFFF22");
		} else {
			item->color = QColor("#000000");
		}
		return item;
	}
	
	void sync() {
		for(auto &p : items) {
			p.second->exists = false;
		}
		
		world->access.lock();
		{
			auto &raw_items = world->items;
			for(auto &rp : raw_items) {
				int id = rp.first;
				Item *ri = rp.second;
				ItemView *iv = nullptr;
				auto ii = items.find(id);
				if(ii == items.end()) {
					iv = instance(ri);
					ii = items.insert(std::pair<int, ItemView*>(id, iv)).first;
					addItem(iv);
				} else {
					iv = ii->second;
				}
				sync_item(ri, iv);
				iv->exists = true;
			}
		}
		world->access.unlock();
		
		for(auto ii = items.begin(); ii != items.end();) {
			if(ii->second->exists) {
				++ii;
			} else {
				auto iv = ii->second;
				removeItem(iv);
				items.erase(ii++);
			}
		}
		
		
	}
	
	virtual bool event(QEvent *event) override {
		if(event->type() == QEvent::User) {
			UserEvent *ue = static_cast<UserEvent*>(event);
			if(ue->utype == SyncEvent::UTYPE) {
				sync();
				return true;
			}
			return false;
		}
		return QGraphicsScene::event(event);
	}
};

class MainView : public QGraphicsView {
public:
	float zf = 1.25;

	MainView() : QGraphicsView() {
		setStyleSheet( "border-style: none;");

		setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
		setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

		setDragMode(ScrollHandDrag);
		
		setRenderHint(QPainter::Antialiasing);
	}

	virtual void enterEvent(QEvent *event) override {
		QGraphicsView::enterEvent(event);
		viewport()->setCursor(Qt::ArrowCursor);
	}
	
	virtual void mousePressEvent(QMouseEvent *event) override {
		QGraphicsView::mousePressEvent(event);
		viewport()->setCursor(Qt::ArrowCursor);
	}
	
	virtual void mouseReleaseEvent(QMouseEvent *event) override {
		QGraphicsView::mouseReleaseEvent(event);
		viewport()->setCursor(Qt::ArrowCursor);
	}
	
	virtual void mouseDoubleClickEvent(QMouseEvent *event) override {
		QGraphicsView::mouseDoubleClickEvent(event);
	}
	
	virtual void wheelEvent(QWheelEvent *event) override {
		float z = pow(zf, event->delta()/120.0);
		scale(z,z);
	}
};

class SidePanel : public QWidget {
public:
	World *world;
	
	QLabel lwsize;
	
	QVBoxLayout layout;
	
	SidePanel(World *w) : QWidget() {
		world = w;
		
		lwsize.setText(("World size: " + std::to_string(world->size)).c_str());
		layout.addWidget(&lwsize);
		
		layout.addStretch(1);
		
		setLayout(&layout);
	}
};

class MainWindow : public QWidget {
public:
	MainScene scene;
	MainView view;

	SidePanel panel;

	QHBoxLayout layout;
	
	MainWindow(World *w) : QWidget(), scene(w), panel(w) {
		view.setScene(&scene);

		layout.addWidget(&view, 2);
		layout.addWidget(&panel, 1);

		setLayout(&layout);
		
		resize(900, 600);
		setWindowTitle("Evolution");
	}
};

int main(int argc, char *argv[]) {
	World world;
	
	QApplication app(argc, argv);
	
	MainWindow window(&world);
	MainScene &scene = window.scene;
	
	window.show();
	
	world.sync = [&app, &scene]() {
		app.postEvent(&scene, new SyncEvent());
	};
	std::thread thread([&world](){world();});

	int rs = app.exec();

	world.done = true;
	thread.join();

	return rs;
}
