#pragma once

#include <QtGui/QtEvents>

class UserEvent : public QEvent {
public:
	int utype;
	UserEvent(int ut = 0) : QEvent(QEvent::User), utype(ut) {}
};

class SyncEvent : public UserEvent {
public:
	static const int UTYPE = 1;
	SyncEvent() : UserEvent(UTYPE) {}
};
