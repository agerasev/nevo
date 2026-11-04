use anyhow::{Result, ensure};

/// A stable identity shared through observations and actions, serialized as an integer.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Handle(u64);

impl Handle {
    pub fn serial(self) -> u64 {
        self.0
    }
}

/// Monotonic allocation never reuses an identity after its owner dies.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandleAllocator {
    next: u64,
}

impl Default for HandleAllocator {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl HandleAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self) -> Result<Handle> {
        self.validate()?;
        let next = self
            .next
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("handle allocator exhausted"))?;
        let handle = Handle(self.next);
        self.next = next;
        Ok(handle)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.next != 0, "handle allocator starts at zero");
        Ok(())
    }

    pub fn validate_handle(&self, handle: Handle) -> Result<()> {
        self.validate()?;
        ensure!(
            handle.0 != 0 && handle.0 < self.next,
            "handle {handle:?} was not allocated"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_monotonic_integers_and_allocation_survives_serialization() {
        let mut allocator = HandleAllocator::new();
        let first = allocator.allocate().unwrap();
        let second = allocator.allocate().unwrap();
        assert!(first < second);
        assert_eq!((first.serial(), second.serial()), (1, 2));

        let mut bytes = Vec::new();
        ciborium::into_writer(&first, &mut bytes).unwrap();
        assert_eq!(
            ciborium::from_reader::<u64, _>(bytes.as_slice()).unwrap(),
            1
        );
        bytes.clear();
        ciborium::into_writer(&allocator, &mut bytes).unwrap();
        let mut restored: HandleAllocator = ciborium::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(restored.allocate().unwrap().serial(), 3);
        assert!(restored.validate_handle(first).is_ok());
        assert!(restored.validate_handle(Handle(0)).is_err());
        assert!(restored.validate_handle(Handle(4)).is_err());
    }

    #[test]
    fn invalid_or_exhausted_allocator_returns_errors_without_wrapping() {
        assert!(HandleAllocator { next: 0 }.validate().is_err());
        let mut allocator = HandleAllocator { next: u64::MAX };
        assert!(allocator.allocate().is_err());
        assert_eq!(allocator.next, u64::MAX);
    }
}
