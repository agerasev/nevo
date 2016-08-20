#pragma once

#include "sidepanel.hpp"
#include "myscene.hpp"

class Window : public QWidget {
public:
	MyScene scene;
	View view;

	SidePanel panel;

	QHBoxLayout layout;
	
	Window(World *w) : QWidget(), scene(w), panel(w) {
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
