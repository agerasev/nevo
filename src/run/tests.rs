use super::*;
use crate::agent::AgentId;

struct World(u64);

struct LockCheckingAgent {
    world: Option<WorldView<World>>,
}

impl Population<u64, u64, ()> for LockCheckingAgent {
    fn spawn(&mut self, _: AgentId, _: ()) {}
    fn despawn(&mut self, _: AgentId) {}

    fn act(&mut self, _: AgentId, observation: &u64) -> u64 {
        let world = self.world.as_ref().unwrap().0.try_write();
        assert!(world.is_ok(), "agent processing must hold no world lock");
        assert_eq!(world.unwrap().0, *observation);
        observation + 1
    }
}

impl Environment<LockCheckingAgent> for World {
    type Observation = u64;
    type Action = u64;
    type Origin = ();
    type Report = u64;

    fn observe(&self) -> Vec<(AgentId, u64)> {
        vec![(AgentId(1), self.0)]
    }

    fn update(
        &mut self,
        _: &mut LockCheckingAgent,
        responses: Vec<AgentResponse<u64, u64>>,
    ) -> u64 {
        self.0 = responses[0].action;
        self.0
    }
}

#[test]
fn agent_work_and_after_tick_callback_hold_no_world_lock() {
    let mut run = Run::from_parts(World(0), LockCheckingAgent { world: None });
    run.population.world = Some(run.world.clone());
    let (_control, commands) = Control::channel(false);
    assert_eq!(
        run.execute(
            LoopOptions {
                ticks: Some(1),
                interval: Duration::ZERO,
            },
            commands,
            |run, report| {
                assert_eq!(run.world.0.try_write().unwrap().0, *report);
                false
            },
        ),
        StopReason::TickLimit,
    );
}
