#!/usr/bin/python3

from PyQt4.QtGui import *
from PyQt4.Qt import *

from world import Entity, Plant, Animal
from event import SyncEvent


def qmix(a, b, r=0.5):
	return QColor(
		a.red()*r + b.red()*(1 - r),
		a.green()*r + b.green()*(1 - r),
		a.blue()*r + b.blue()*(1 - r),
		255
	)


class Item(QGraphicsItem):
	def __init__(self, entity):
		QGraphicsItem.__init__(self)
		self.exist = True

		self.color = QColor("#000000")
		if isinstance(entity, Plant):
			self.color = QColor("#22CC22")
		elif isinstance(entity, Animal):
			self.color = QColor("#FFFF22")

		self.sync(entity)

	def sync(self, entity):
		self.setPos(QPointF(entity.pos[0], entity.pos[1]))
		self.size = entity.size
		self.update()

	def boundingRect(self):
		return QRectF(-self.size, -self.size, 2*self.size, 2*self.size)

	def paint(self, painter, option, widget=0):
		if self.size > 0.0:
			pen = QPen()
			pen.setWidth(2)
			pen.setCosmetic(True)
			pen.setColor(qmix(self.color, QColor("#000000"), 0.75))
			painter.setPen(pen)

			painter.setBrush(self.color)
			painter.drawEllipse(self.boundingRect())


class MainScene(QGraphicsScene):
	def __init__(self, world):
		QGraphicsScene.__init__(self)
		self.world = world
		self.items = {}
		self.setBackgroundBrush(QColor("#FFFFFF"))
		self.setSceneRect(-0.5*world.size[0], -0.5*world.size[1], world.size[0], world.size[1])
		self.addRect(self.sceneRect(), QPen(), QBrush(QColor("#FFFFCC")))

	def sync(self):
		for key, item in self.items.items():
			item.exist = False

		with self.world.access:
			for key, entity in self.world.entities.items():
				item = self.items.get(key, None)
				if item is None:
					item = Item(entity)
					self.items[key] = item
					item.setZValue(key)
					self.addItem(item)
				else:
					item.sync(entity)
				item.exist = True

		delkeys = []
		for key, item in self.items.items():
			if not item.exist:
				delkeys.append(key)
		for key in delkeys:
			del self.items[key]

	def event(self, event):
		if event.type() == QEvent.User:
			if isinstance(event, SyncEvent):
				self.sync()
				return True
			return False
		return QGraphicsScene.event(self, event)


class MainView(QGraphicsView):
	def __init__(self):
		QGraphicsView.__init__(self)

		self.fzoom = 1.25

		self.setStyleSheet("border-style: none;")

		self.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
		self.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)

		self.setDragMode(self.ScrollHandDrag)

		self.setRenderHint(QPainter.Antialiasing)

	def enterEvent(self, event):
		QGraphicsView.enterEvent(self, event)
		self.viewport().setCursor(Qt.ArrowCursor)

	def mousePressEvent(self, event):
		QGraphicsView.mousePressEvent(self, event)
		self.viewport().setCursor(Qt.ArrowCursor)

	def mouseReleaseEvent(self, event):
		QGraphicsView.mouseReleaseEvent(self, event)
		self.viewport().setCursor(Qt.ArrowCursor)

	def mouseDoubleClickEvent(self, event):
		QGraphicsView.mouseDoubleClickEvent(self, event)

	def wheelEvent(self, event):
		z = pow(self.fzoom, event.delta()/120)
		self.scale(z, z)


class SidePanel(QWidget):
	def __init__(self, world):
		QWidget.__init__(self)
		self.world = world

		vbox = QVBoxLayout()
		vbox.addWidget(QLabel('world size: %dx%d' % (world.size[0], world.size[1])))
		vbox.addStretch(1)
		self.setLayout(vbox)


class MainWindow(QWidget):
	def __init__(self, world):
		QWidget.__init__(self)

		self.scene = MainScene(world)
		self.view = MainView()
		self.view.setScene(self.scene)

		self.panel = SidePanel(world)

		hbox = QHBoxLayout()
		hbox.addWidget(self.view, 2)
		hbox.addWidget(self.panel, 1)
		self.setLayout(hbox)

		# construct window
		self.resize(1280, 720)
		self.setWindowTitle('NEvo')
