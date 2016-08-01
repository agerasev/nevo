#include "random.hpp"

#include <random>

static std::minstd_rand rand_engine;
static std::uniform_real_distribution<> unif_dist;
static std::normal_distribution<> norm_dist;


double randu() {
	return unif_dist(rand_engine);
}

double randn() {
	return norm_dist(rand_engine);
}

vec2 randu2() {
	return vec2(randu(), randu());
}

