#pragma once

#include <QGraphicsItem>

#include <la/vec.hpp>

#include <world/entity.hpp>
#include <world/spawn.hpp>


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
	constexpr static const char *COLOR = "#888888";
	
	bool exists = true;
	
	double size = 20.0;
	
	QColor color;
	
	Item(Entity *e) : QGraphicsItem() {
		color = QColor(COLOR);
	}
	virtual ~Item() = default;
	
	virtual QRectF boundingRect() const {
		return QRectF(-size, -size, 2*size, 2*size);
	}
	
	void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget = 0) override {
		if(size > 0.0) {
			QColor dark_color = qmix(color, QColor("#000000"), 0.75);
			QPen pen;
			pen.setWidth(2);
			pen.setCosmetic(true);
			pen.setColor(dark_color);
			painter->setPen(pen);
			
			painter->setBrush(color);
			painter->drawEllipse(boundingRect());
		}
	}
	
	virtual void sync(Entity *e) {
		setPos(v2q(e->pos));
		size = e->size();
		// update();
	}
};

class ItemPlant : public Item {
public:
	constexpr static const char *COLOR = "#22CC22";
	
	ItemPlant(Plant *p) : Item(p) {
		color = QColor(COLOR);
	}
};

class ItemAnimal : public Item {
public:
	constexpr static const char 
		*ACOLOR = "#CCCCCC",
		*HCOLOR = "#FFFF22",
		*CCOLOR = "#FF2222";
	
	vec2 dir = vec2(1, 0);
	
	ItemAnimal(Animal *a) : Item(a) {
		if(dynamic_cast<Herbivore*>(a)) {
			color = QColor(HCOLOR);
		} else if(dynamic_cast<Carnivore*>(a)) {
			color = QColor(CCOLOR);
		} else {
			color = QColor(ACOLOR);
		}
	}
	
	void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget) override {
		Item::paint(painter, option, widget);
		painter->drawLine(v2q(nullvec2), v2q(size*dir));
	}
	
	void sync(Entity *e) override {
		Item::sync(e);
		dir = dynamic_cast<Animal*>(e)->dir;
	}
};

class ItemSpawn : public Item {
public:
	ItemSpawn(Spawn *s) : Item(s) {
		if(dynamic_cast<SpawnAnimal*>(s)) {
			if(dynamic_cast<SpawnHerbivore*>(s)) {
				color = QColor(ItemAnimal::HCOLOR);
			} else if(dynamic_cast<SpawnCarnivore*>(s)) {
				color = QColor(ItemAnimal::CCOLOR);
			} else {
				color = QColor(ItemAnimal::ACOLOR);
			}
		} else if(dynamic_cast<SpawnPlant*>(s)) {
			color = QColor(ItemPlant::COLOR);
		} else {
			color = QColor(Item::COLOR);
		}
	}
	
	void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget = 0) override {
		if(size > 0.0) {
			QColor c = color;
			c.setAlpha(127);
			QPen pen;
			pen.setWidth(4);
			pen.setCosmetic(true);
			pen.setColor(c);
			pen.setStyle(Qt::DotLine);
			painter->setPen(pen);
			painter->drawEllipse(boundingRect());
		}
	}
};
