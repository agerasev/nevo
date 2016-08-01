cmake_minimum_required(VERSION 2.6)
project(nevo)

set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Wall -fPIC")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${CMAKE_C_FLAGS} -std=c++11 -fno-exceptions")

set(SOURCE source/world/random.cpp)

set(QT5_INCLUDE /usr/include/x86_64-linux-gnu/qt5)
include_directories(source libla ${QT5_INCLUDE} ${QT5_INCLUDE}/QtWidgets)

add_executable(nevo source/main.cpp ${SOURCE})

set(LIBS ${LIBS} pthread)
set(LIBS ${LIBS} Qt5Core Qt5Gui Qt5Widgets)

target_link_libraries(nevo ${LIBS})
