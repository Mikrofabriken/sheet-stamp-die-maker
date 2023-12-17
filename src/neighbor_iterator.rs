use core::slice;
use ordered_float::NotNan;
use std::collections::BTreeMap;

/// Integer coordinates
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Coordinate {
    pub x: i32,
    pub y: i32,
}

/// Type that allows iteration over neighbors of pixels (integer coordinates) ordered by distance
/// starting with the closest neighbor and ending at a predefined max coordinate.
///
/// The coordinate system has positive Y going *down* and positive X going to the *right*.
pub struct Neighbors {
    neighbors: Vec<(Coordinate, f32)>,
}

impl Neighbors {
    pub fn new(max_radius: f32) -> Self {
        // We need positive radius. Too lazy to create an error type, just panic
        assert!(max_radius >= 0.0);

        let mut neighbors = BTreeMap::new();

        // Radius squared. Will be used many times, so pre-compute it.
        let r2 = max_radius.powi(2);

        let end_y = max_radius.floor() as i32;
        let start_y = -end_y;
        for y in start_y..=end_y {
            let end_x = (r2 - (y as f32).powi(2)).sqrt().floor() as i32;
            let start_x = -end_x;
            for x in start_x..=end_x {
                let distance = ((x * x + y * y) as f32).sqrt();
                let distance = NotNan::new(distance).expect("Side of triangle can't be NaN");
                let pos = Coordinate { x, y };
                neighbors
                    .entry(distance)
                    .or_insert_with(|| Vec::new())
                    .push((pos, distance.into_inner()));
            }
        }

        // Take out all the sorted (Coordinate, float) values from the BTreeMap. Flatten
        // since each entry (distance) can have multiple entries.
        let neighbors = neighbors.values().flatten().copied().collect();
        Neighbors { neighbors }
    }
}

pub struct NeighborIterator<'a> {
    neighbors: slice::Iter<'a, (Coordinate, f32)>,
}

impl<'a> Iterator for NeighborIterator<'a> {
    type Item = (Coordinate, f32);

    fn next(&mut self) -> Option<Self::Item> {
        self.neighbors.next().copied()
    }
}

impl<'a> IntoIterator for &'a Neighbors {
    type Item = (Coordinate, f32);

    type IntoIter = NeighborIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        NeighborIterator {
            neighbors: self.neighbors.iter(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let testee = Neighbors::new(0.99);
        let neighbors: Vec<_> = testee.into_iter().collect();
        assert_eq!(vec![(Coordinate { x: 0, y: 0 }, 0.0)], neighbors);
    }

    #[test]
    fn tiny() {
        let testee = Neighbors::new(1.0);
        let neighbors: Vec<_> = testee.into_iter().collect();
        assert_eq!(
            vec![
                (Coordinate { x: 0, y: 0 }, 0.0),
                (Coordinate { x: 0, y: -1 }, 1.0),
                (Coordinate { x: -1, y: 0 }, 1.0),
                (Coordinate { x: 1, y: 0 }, 1.0),
                (Coordinate { x: 0, y: 1 }, 1.0)
            ],
            neighbors
        );

        // Just below  where (-1, -1), (1, -1), (-1, 1) and (1, 1) would be included
        let just_below_two = Neighbors::new(1.9999f32.sqrt());
        let just_below_two_neighbors: Vec<_> = just_below_two.into_iter().collect();
        assert_eq!(just_below_two_neighbors, neighbors);
    }

    #[test]
    fn sqrt_2() {
        let sqrt_2 = 2.0f32.sqrt();
        // Make the radius sliightly larger than sqrt(2) to include (-1, -1) etc
        let testee = Neighbors::new(sqrt_2 + 0.0001);
        let neighbors: Vec<_> = testee.into_iter().collect();
        assert_eq!(
            vec![
                // Center
                (Coordinate { x: 0, y: 0 }, 0.0),
                // Distance 1
                (Coordinate { x: 0, y: -1 }, 1.0),
                (Coordinate { x: -1, y: 0 }, 1.0),
                (Coordinate { x: 1, y: 0 }, 1.0),
                (Coordinate { x: 0, y: 1 }, 1.0),
                // Corners
                (Coordinate { x: -1, y: -1 }, sqrt_2),
                (Coordinate { x: 1, y: -1 }, sqrt_2),
                (Coordinate { x: -1, y: 1 }, sqrt_2),
                (Coordinate { x: 1, y: 1 }, sqrt_2),
            ],
            neighbors
        );
    }
}
