#!/bin/sh

mkdir -p build
cd build
cmake -D CMAKE_C_COMPILER=clang -D CMAKE_CXX_COMPILER=clang++ ..
# cmake ..
make
