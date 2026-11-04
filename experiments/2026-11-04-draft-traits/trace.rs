use nevo::{foraging::Scenario, run::Run};

fn snapshot(run: &Run) {
    println!("world={:?}", run.world);
    for id in run.world.animals().keys() {
        println!("agent={:?} {:?}", id, run.population.get(*id).unwrap());
    }
    // Compare RNG state independently of its new CPU-context wrapper.
    let population = format!("{:?}", run.population);
    let start = population
        .find("Xoshiro256PlusPlus {")
        .expect("genetic RNG");
    let end = start + population[start..].find('}').expect("RNG end") + 1;
    println!("genetic_rng={}", &population[start..end]);
}

fn main() {
    for scenario in [Scenario::Debug, Scenario::Baseline] {
        for seed in [1, 42] {
            println!("run={scenario:?} seed={seed}");
            let mut run = Run::new(scenario, seed);
            snapshot(&run);
            for _ in 0..128 {
                println!("report={:?}", run.tick());
                snapshot(&run);
            }
        }
    }
}
