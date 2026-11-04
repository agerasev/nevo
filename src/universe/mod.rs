//! Complete simulation state and observation/action orchestration.

pub mod foraging;

use anyhow::{Result, ensure};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::{
    actor::{Actor, HandleAllocator, herbivore::Herbivores},
    context::{Context, Cpu},
    environment::{Environment, ecosystem::Ecosystem},
};
pub use foraging::ForagingPolicy;

/// Experiment interventions, separate from physical rules and controller logic.
pub trait Policy<Request> {
    type Intervention;

    fn respond(&mut self, request: Request) -> Result<Self::Intervention>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Everything needed to continue a simulation. Locks and viewers live outside it.
///
/// Clone must produce independently mutable simulation state. Reconstructible
/// caches may be skipped by serde; RNGs and recurrent memory must be retained.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Universe<E = Ecosystem, A = Herbivores, C = Cpu<Xoshiro256PlusPlus>, P = ForagingPolicy>
{
    pub(crate) step: u64,
    pub(crate) environment: E,
    pub(crate) actor: A,
    pub(crate) context: C,
    pub(crate) policy: P,
    pub(crate) handles: HandleAllocator,
}

impl<E, A, C, P> Universe<E, A, C, P> {
    pub fn step(&self) -> u64 {
        self.step
    }

    pub fn environment(&self) -> &E {
        &self.environment
    }

    pub fn actor(&self) -> &A {
        &self.actor
    }

    pub fn context(&self) -> &C {
        &self.context
    }

    pub fn policy(&self) -> &P {
        &self.policy
    }

    pub fn handles(&self) -> &HandleAllocator {
        &self.handles
    }
}

impl<E, A, C, P> Universe<E, A, C, P>
where
    E: Environment + Clone,
    A: Actor<C, Observation = E::Observation, Action = E::Action> + Clone,
    C: Context + Clone,
    P: Policy<E::Request, Intervention = E::Intervention> + Clone,
{
    pub fn from_parts(
        step: u64,
        environment: E,
        actor: A,
        context: C,
        policy: P,
        handles: HandleAllocator,
    ) -> Result<Self> {
        let universe = Self {
            step,
            environment,
            actor,
            context,
            policy,
            handles,
        };
        universe.validate()?;
        Ok(universe)
    }

    fn validate_components(&self) -> Result<()> {
        ensure!(self.step < u64::MAX, "universe step counter is exhausted");
        self.handles.validate()?;
        self.environment.validate(&self.handles)?;
        self.actor.validate()?;
        self.policy.validate()?;
        Ok(())
    }

    /// Validate components and their observation protocol without altering state.
    pub fn validate(&self) -> Result<()> {
        self.validate_components()?;
        let mut actor = self.actor.clone();
        let mut context = self.context.clone();
        actor.observe(&mut context, self.environment.observe())?;
        Ok(())
    }

    /// Prepare an independent complete step. Failure leaves this universe intact.
    pub fn prepare_step(&self) -> Result<(Self, E::Report)> {
        let mut next = self.clone();
        let step = self
            .step
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("universe step counter is exhausted"))?;
        next.actor
            .observe(&mut next.context, next.environment.observe())?;
        let action = next.actor.act(&mut next.context)?;
        let (report, feedback) =
            next.environment
                .advance(step, action, &mut next.handles, |request| {
                    next.policy.respond(request)
                })?;
        // Feedback may synchronize membership without causing another action.
        next.actor.observe(&mut next.context, feedback)?;
        next.step = step;
        next.validate_components()?;
        Ok((next, report))
    }

    pub fn advance(&mut self) -> Result<E::Report> {
        let (next, report) = self.prepare_step()?;
        *self = next;
        Ok(report)
    }
}
