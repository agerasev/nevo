use std::ops::{Index, IndexMut};

use anyhow::{bail, Result};
use glam::UVec2;

#[derive(Clone, Debug)]
pub struct Grid2<T> {
    size: UVec2,
    items: Vec<T>,
}

impl<T> Grid2<T> {
    pub fn new(size: UVec2, items: Vec<T>) -> Result<Self> {
        if size.x as usize * size.y as usize != items.len() {
            bail!("Size {size} do not match number of items {}", items.len());
        }
        Ok(Self { size, items })
    }
    pub fn fill(size: UVec2, item: T) -> Self
    where
        T: Clone,
    {
        Self {
            size,
            items: vec![item; size.x as usize * size.y as usize],
        }
    }
    pub fn fill_with<F: FnMut(UVec2) -> T>(size: UVec2, f: F) -> Self
    where
        T: Clone,
    {
        Self {
            size,
            items: (0..size.y)
                .flat_map(|y| (0..size.x).map(move |x| UVec2::new(x, y)))
                .map(f)
                .collect(),
        }
    }

    pub fn get(&self, index: UVec2) -> Option<&T> {
        if index.y < self.size.y {
            self.items
                .get(index.y as usize * self.size.x as usize + index.x as usize)
        } else {
            None
        }
    }
    pub fn get_mut(&mut self, index: UVec2) -> Option<&mut T> {
        if index.y < self.size.y {
            self.items
                .get_mut(index.y as usize * self.size.x as usize + index.x as usize)
        } else {
            None
        }
    }

    pub fn size(&self) -> UVec2 {
        self.size
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }
    pub fn items_mut(&self) -> &[T] {
        &self.items
    }
}

impl<T> Index<UVec2> for Grid2<T> {
    type Output = T;
    fn index(&self, index: UVec2) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("Grid index ({index}) is out of bounds ({})", self.size))
    }
}

impl<T> IndexMut<UVec2> for Grid2<T> {
    fn index_mut(&mut self, index: UVec2) -> &mut Self::Output {
        let size = self.size;
        self.get_mut(index)
            .unwrap_or_else(|| panic!("Grid index ({index}) is out of bounds ({size})"))
    }
}
