#!/usr/bin/python3

from PyQt4.Qt import QEvent


class UserEvent(QEvent):
	def __init__(self):
		QEvent.__init__(self, QEvent.User)


class SyncEvent(UserEvent):
	def __init__(self):
		UserEvent.__init__(self)
