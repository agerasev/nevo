cmake_minimum_required(VERSION 2.6)
project(nevo)

set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Wall")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${CMAKE_C_FLAGS} -std=c++11")

set(SOURCE "")

include_directories(source libla)
if(${WIN32})
	include_directories(${QT4_INCLUDE_DIR})
else()
	include_directories(/usr/include/qt4)
endif()

add_executable(nevo source/main.cpp ${SOURCE})

set(LIBS ${LIBS} pthread)
set(LIBS ${LIBS} QtCore QtGui QtSvg)

target_link_libraries(nevo ${LIBS})
