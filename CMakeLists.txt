cmake_minimum_required(VERSION 2.6)
project(nevo)

set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Wall")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${CMAKE_C_FLAGS} -std=c++11")

set(SOURCE "")

include_directories(source /usr/include/qt4)

add_executable(nevo source/main.cpp ${SOURCE})

set(LIBS QtCore QtGui)

target_link_libraries(nevo ${LIBS})
