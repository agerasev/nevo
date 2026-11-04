//! Ordered collections of actors using an explicit membership protocol.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::{Actor, Handle};
use crate::context::Context;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Lifecycle<S> {
    Spawn { handle: Handle, origin: S },
    Despawn { handle: Handle },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PopulationObservation<O, S> {
    Inputs {
        members: Vec<Handle>,
        inputs: Vec<(Handle, O)>,
    },
    Lifecycle {
        members: Vec<Handle>,
        changes: Vec<Lifecycle<S>>,
    },
}

pub type PopulationAction<A> = Vec<(Handle, A)>;

/// Interpretation of a creation spec belongs to the actor implementation.
pub trait Factory<C: Context + ?Sized, A> {
    type Origin;

    fn create(&self, cx: &mut C, origin: Self::Origin, existing: &BTreeMap<Handle, A>)
    -> Result<A>;

    fn validate(&self, _existing: &BTreeMap<Handle, A>) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Population<A, F> {
    live: BTreeMap<Handle, A>,
    factory: F,
    #[serde(default)]
    pending: Option<Vec<Handle>>,
}

impl<A, F> Population<A, F> {
    pub fn new(factory: F) -> Self {
        Self {
            live: BTreeMap::new(),
            factory,
            pending: None,
        }
    }

    pub fn get(&self, handle: Handle) -> Option<&A> {
        self.live.get(&handle)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle, &A)> + '_ {
        self.live.iter().map(|(handle, actor)| (*handle, actor))
    }

    pub fn members(&self) -> impl Iterator<Item = Handle> + '_ {
        self.live.keys().copied()
    }

    pub(crate) fn from_parts(live: BTreeMap<Handle, A>, factory: F) -> Self {
        Self {
            live,
            factory,
            pending: None,
        }
    }

    fn check_members(&self, members: Vec<Handle>) -> Result<()> {
        let count = members.len();
        let members: BTreeSet<_> = members.into_iter().collect();
        ensure!(count == members.len(), "duplicate population member");
        ensure!(
            self.live.keys().copied().eq(members),
            "population membership does not match environment"
        );
        Ok(())
    }
}

impl<A, F: Default> Default for Population<A, F> {
    fn default() -> Self {
        Self::new(F::default())
    }
}

impl<C, A, F> Actor<C> for Population<A, F>
where
    C: Context + ?Sized,
    A: Actor<C>,
    F: Factory<C, A>,
{
    type Observation = PopulationObservation<A::Observation, F::Origin>;
    type Action = PopulationAction<A::Action>;

    fn observe(&mut self, cx: &mut C, observation: Self::Observation) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "population still has pending actions"
        );
        match observation {
            PopulationObservation::Inputs { members, inputs } => {
                self.check_members(members)?;
                let mut unique = BTreeSet::new();
                for (handle, _) in &inputs {
                    ensure!(self.live.contains_key(handle), "unknown actor {handle:?}");
                    ensure!(unique.insert(*handle), "duplicate input for {handle:?}");
                }
                let mut pending = Vec::with_capacity(inputs.len());
                for (handle, observation) in inputs {
                    self.live
                        .get_mut(&handle)
                        .ok_or_else(|| anyhow::anyhow!("unknown actor {handle:?}"))?
                        .observe(cx, observation)?;
                    pending.push(handle);
                }
                self.pending = Some(pending);
            }
            PopulationObservation::Lifecycle { members, changes } => {
                for change in changes {
                    match change {
                        Lifecycle::Spawn { handle, origin } => {
                            ensure!(
                                handle.serial() != 0 && !self.live.contains_key(&handle),
                                "invalid or duplicate spawn {handle:?}"
                            );
                            let actor = self.factory.create(cx, origin, &self.live)?;
                            self.live.insert(handle, actor);
                        }
                        Lifecycle::Despawn { handle } => {
                            ensure!(
                                self.live.remove(&handle).is_some(),
                                "unknown actor {handle:?}"
                            );
                        }
                    }
                }
                self.check_members(members)?;
            }
        }
        Ok(())
    }

    fn act(&mut self, cx: &mut C) -> Result<Self::Action> {
        let pending = self
            .pending
            .take()
            .ok_or_else(|| anyhow::anyhow!("population has no pending inputs"))?;
        pending
            .into_iter()
            .map(|handle| {
                let action = self
                    .live
                    .get_mut(&handle)
                    .ok_or_else(|| anyhow::anyhow!("unknown actor {handle:?}"))?
                    .act(cx)?;
                Ok((handle, action))
            })
            .collect()
    }

    fn validate(&self) -> Result<()> {
        for (handle, actor) in &self.live {
            ensure!(handle.serial() != 0, "invalid zero actor handle");
            actor.validate()?;
        }
        if let Some(pending) = &self.pending {
            let mut unique = BTreeSet::new();
            for handle in pending {
                ensure!(self.live.contains_key(handle), "unknown actor {handle:?}");
                ensure!(unique.insert(*handle), "duplicate pending actor {handle:?}");
            }
        }
        self.factory.validate(&self.live)
    }
}

#[cfg(test)]
mod tests;
