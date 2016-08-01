#pragma once

#include <QWidget>
#include <QGraphicsView>
#include <QHBoxLayout>
#include <QVBoxLayout>

#include <map>
#include <cmath>
#include <string>

#include <world/world.hpp>
#include <event.hpp>

#include "sidepanel.hpp"
#include "item.hpp"

class MainScene : public QGraphicsScene {
public:
	std::map<long, Item*> items;
	World *world = nullptr;

	MainScene(World *w) : QGraphicsScene() {
		world = w;
		
		setBackgroundBrush(QColor("#FFFFFF"));
		setSceneRect(-world->size.x(), -world->size.y(), 2*world->size.x(), 2*world->size.y());
		addRect(sceneRect(), QPen(), QBrush(QColor("#FFFFCC")));
	}

	virtual ~MainScene() {
		for(auto &p : items) {
			delete p.second;
		}
	}
	
	Item *instance(Entity *e) const {
		if(auto p = dynamic_cast<Plant*>(e))
			return new ItemPlant(p);
		if(auto a = dynamic_cast<Animal*>(e))
			return new ItemAnimal(a);
		if(auto s = dynamic_cast<Spawn*>(e))
			return new ItemSpawn(s);
		return new Item(e);
	}
	
	void sync() {
		for(auto &p : items) {
			p.second->exists = false;
		}
		
		world->access.lock();
		{
			for(auto &ep : world->entities) {
				int id = ep.first;
				Entity *e = ep.second;
				Item *it = nullptr;
				auto ii = items.find(id);
				if(ii == items.end()) {
					it = instance(e);
					it->setZValue(id);
					ii = items.insert(std::pair<int, Item*>(id, it)).first;
					addItem(it);
				} else {
					it = ii->second;
				}
				it->sync(e);
				it->exists = true;
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
				delete iv;
			}
		}
		
		update();
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
		
		resize(1280, 720);
		setWindowTitle("Evolution");
		
		vec2 zv = vec2(view.size().width(), view.size().height())/(2*w->size);
		float z = zv.x() > zv.y() ? zv.x() : zv.y();
		view.scale(z, z);
	}
	
	virtual bool event(QEvent *event) override {
		if(event->type() == QEvent::User) {
			UserEvent *ue = static_cast<UserEvent*>(event);
			if(ue->utype == SyncEvent::UTYPE) {
				scene.sync();
				panel.sync();
				return true;
			}
			return false;
		}
		return QWidget::event(event);
	}
};
