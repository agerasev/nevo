#include "random.hpp"

#include <random>
#include <cmath>

static std::minstd_rand rand_engine;
static std::uniform_int_distribution<> int_dist;
static std::uniform_real_distribution<> unif_dist;
static std::normal_distribution<> norm_dist;

int rand_int() {
	return int_dist(rand_engine);
}

double rand_unif() {
	return unif_dist(rand_engine);
}

double rand_norm() {
	return norm_dist(rand_engine);
}

vec2 rand_unif2() {
	return vec2(rand_unif(), rand_unif());
}

vec2 rand_norm2() {
	return vec2(rand_norm(), rand_norm());
}

vec2 rand_circle() {
	double a = 2*M_PI*rand_unif();
	return vec2(cos(a), sin(a));
}

vec2 rand_disk() {
	return sqrt(rand_unif())*rand_circle();
}
