#!/usr/bin/python3

import sys
from threading import Thread

from PyQt4.Qt import QApplication

from world import World
from graphics import MainWindow
from event import SyncEvent

world = World()

app = QApplication(sys.argv)

window = MainWindow(world)
window.show()

world.sync = lambda: app.postEvent(window.scene, SyncEvent())
thread = Thread(target=world)
thread.start()

rs = app.exec_()

world.done = True
thread.join()

sys.exit(rs)
