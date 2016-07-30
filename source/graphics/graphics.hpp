#pragma once

#include <QWidget>
#include <QGraphicsItem>
#include <QGraphicsView>
#include <QHBoxLayout>
#include <QVBoxLayout>

#include <map>
#include <cmath>
#include <string>

#include <world/world.hpp>
#include <event.hpp>

#include "sidepanel.hpp"


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

class Item : public QGraphicsItem {
public:
	enum class Type {
		NONE = 0,
		PLANT,
		ANIMAL,
		AREA
	};

	Type type = Type::NONE;
	
	bool exists = true;
	
	double size = 20.0;
	vec2 dir = vec2(1, 0);
	
	QColor color;
	
	Item(Entity *e) : QGraphicsItem() {
		if(dynamic_cast<Plant*>(e)) {
			type = Type::PLANT;
			color = QColor("#22CC22");
		} else if(dynamic_cast<Animal*>(e)) {
			type = Type::ANIMAL;
			color = QColor("#FFFF22");
		} else {
			type = Type::NONE;
			color = QColor("#000000");
		}
	}
	
	virtual QRectF boundingRect() const {
		return QRectF(-size, -size, 2*size, 2*size);
	}
	
	virtual void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget = 0) {
		if(size > 0.0) {
			QColor dark_color = qmix(color, QColor("#000000"), 0.75);
			QPen pen;
			pen.setWidth(2);
			pen.setCosmetic(true);
			pen.setColor(dark_color);
			painter->setPen(pen);
			
			painter->setBrush(color);
			painter->drawEllipse(boundingRect());
			if(type == Type::ANIMAL)
				painter->drawLine(v2q(nullvec2), v2q(size*dir));
		}
	}
	
	void sync(Entity *e) {
		setPos(v2q(e->pos));
		size = e->size();
		dir = e->dir;
		// item->update();
	}
};

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
					it = new Item(e);
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
