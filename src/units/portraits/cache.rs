//! Portrait image cache, request queue, and generation coalescing.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

use crate::world::UnitId;

/// Stable appearance fingerprint for one unit portrait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortraitAppearanceSignature {
    pub unit_id: UnitId,
    pub digest: u64,
}

#[derive(Debug, Clone)]
pub struct PortraitCacheEntry {
    pub image: Handle<Image>,
    pub signature: PortraitAppearanceSignature,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortraitCaptureRequest {
    pub unit_id: UnitId,
    pub signature: PortraitAppearanceSignature,
    pub generation: u64,
}

/// Client-local portrait cache (rebuilt after load; never serialized).
#[derive(Resource, Debug, Default)]
pub struct UnitPortraitCache {
    pub entries: HashMap<UnitId, PortraitCacheEntry>,
    /// Latest requested generation per unit (monotonic).
    pub requested_generation: HashMap<UnitId, u64>,
    pub queue: VecDeque<PortraitCaptureRequest>,
    pub lru: VecDeque<UnitId>,
    pub max_entries: usize,
}

impl UnitPortraitCache {
    pub const DEFAULT_CAPACITY: usize = 64;

    pub fn image_for_unit(&self, unit_id: UnitId) -> Option<&Handle<Image>> {
        self.entries.get(&unit_id).map(|entry| &entry.image)
    }

    pub fn entry_matches(&self, unit_id: UnitId, signature: &PortraitAppearanceSignature) -> bool {
        self.entries
            .get(&unit_id)
            .is_some_and(|entry| entry.signature == *signature)
    }

    /// Enqueue a capture when the cache is stale. Returns the generation assigned.
    pub fn request_capture(
        &mut self,
        unit_id: UnitId,
        signature: PortraitAppearanceSignature,
    ) -> u64 {
        if self
            .entries
            .get(&unit_id)
            .is_some_and(|entry| entry.signature == signature)
        {
            self.touch_lru(unit_id);
            return self
                .entries
                .get(&unit_id)
                .map(|e| e.generation)
                .unwrap_or(0);
        }
        let generation = self
            .requested_generation
            .get(&unit_id)
            .map(|value| value + 1)
            .unwrap_or(1);
        self.requested_generation.insert(unit_id, generation);
        self.coalesce_queue(unit_id, signature, generation);
        generation
    }

    pub(crate) fn coalesce_queue(
        &mut self,
        unit_id: UnitId,
        signature: PortraitAppearanceSignature,
        generation: u64,
    ) {
        self.queue.retain(|item| item.unit_id != unit_id);
        self.queue.push_back(PortraitCaptureRequest {
            unit_id,
            signature,
            generation,
        });
    }

    pub fn pop_next_request(&mut self) -> Option<PortraitCaptureRequest> {
        self.queue.pop_front()
    }

    pub fn commit_capture(
        &mut self,
        unit_id: UnitId,
        generation: u64,
        signature: PortraitAppearanceSignature,
        image: Handle<Image>,
    ) -> bool {
        if self.requested_generation.get(&unit_id) != Some(&generation) {
            return false;
        }
        while self.lru.len() >= self.max_entries {
            if let Some(evicted) = self.lru.pop_front() {
                if evicted != unit_id {
                    self.entries.remove(&evicted);
                }
            }
        }
        self.entries.insert(
            unit_id,
            PortraitCacheEntry {
                image,
                signature,
                generation,
            },
        );
        self.touch_lru(unit_id);
        true
    }

    pub fn remove_unit(&mut self, unit_id: UnitId) {
        self.entries.remove(&unit_id);
        self.requested_generation.remove(&unit_id);
        self.queue.retain(|item| item.unit_id != unit_id);
        self.lru.retain(|id| *id != unit_id);
    }

    pub fn prune_missing_units(&mut self, alive: impl Fn(UnitId) -> bool) {
        let stale: Vec<UnitId> = self
            .entries
            .keys()
            .filter(|id| !alive(**id))
            .copied()
            .collect();
        for id in stale {
            self.remove_unit(id);
        }
    }

    fn touch_lru(&mut self, unit_id: UnitId) {
        self.lru.retain(|id| *id != unit_id);
        self.lru.push_back(unit_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(unit: u32, digest: u64) -> PortraitAppearanceSignature {
        PortraitAppearanceSignature {
            unit_id: UnitId::new(unit),
            digest,
        }
    }

    #[test]
    fn coalesces_duplicate_queue_entries() {
        let mut cache = UnitPortraitCache::default();
        cache.request_capture(UnitId::new(1), sig(1, 10));
        cache.request_capture(UnitId::new(1), sig(1, 20));
        assert_eq!(cache.queue.len(), 1);
        assert_eq!(cache.queue[0].signature.digest, 20);
        assert_eq!(cache.queue[0].generation, 2);
    }

    #[test]
    fn stale_generation_rejects_commit() {
        let mut cache = UnitPortraitCache::default();
        cache.request_capture(UnitId::new(1), sig(1, 1));
        cache.request_capture(UnitId::new(1), sig(1, 2));
        let image = Handle::default();
        assert!(!cache.commit_capture(UnitId::new(1), 1, sig(1, 1), image.clone()));
        assert!(cache.commit_capture(UnitId::new(1), 2, sig(1, 2), image));
    }

    #[test]
    fn remove_unit_clears_queue() {
        let mut cache = UnitPortraitCache::default();
        cache.request_capture(UnitId::new(3), sig(3, 1));
        cache.remove_unit(UnitId::new(3));
        assert!(cache.queue.is_empty());
        assert!(cache.entries.is_empty());
    }
}
