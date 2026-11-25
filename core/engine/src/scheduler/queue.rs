use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A named download queue with independent concurrency and bandwidth limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedQueue {
    pub id: Uuid,
    pub name: String,
    pub max_concurrent: u32,
    pub max_speed_bps: Option<u64>,
    pub task_ids: Vec<Uuid>,
    pub enabled: bool,
}

/// Manages named download queues, task membership, ordering, and concurrency slots.
#[derive(Debug, Default)]
pub struct QueueManager {
    queues: Vec<NamedQueue>,
    active_tasks: HashMap<Uuid, Vec<Uuid>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_queues(queues: Vec<NamedQueue>) -> Self {
        Self {
            queues,
            active_tasks: HashMap::new(),
        }
    }

    pub fn queues(&self) -> &[NamedQueue] {
        &self.queues
    }

    pub fn get(&self, id: Uuid) -> Option<&NamedQueue> {
        self.queues.iter().find(|q| q.id == id)
    }

    pub fn create(
        &mut self,
        name: impl Into<String>,
        max_concurrent: u32,
        max_speed_bps: Option<u64>,
    ) -> NamedQueue {
        let queue = NamedQueue {
            id: Uuid::new_v4(),
            name: name.into(),
            max_concurrent: max_concurrent.max(1),
            max_speed_bps,
            task_ids: Vec::new(),
            enabled: true,
        };
        self.queues.push(queue.clone());
        queue
    }

    pub fn delete(&mut self, id: Uuid) -> bool {
        let before = self.queues.len();
        self.queues.retain(|q| q.id != id);
        self.active_tasks.remove(&id);
        self.queues.len() != before
    }

    pub fn rename(&mut self, id: Uuid, new_name: impl Into<String>) -> bool {
        if let Some(q) = self.queues.iter_mut().find(|q| q.id == id) {
            q.name = new_name.into();
            true
        } else {
            false
        }
    }

    /// Reorders queues to match `ordered_ids`; any remaining queues keep their relative order.
    pub fn reorder(&mut self, ordered_ids: &[Uuid]) {
        let mut rank: HashMap<Uuid, usize> = HashMap::with_capacity(ordered_ids.len());
        for (idx, id) in ordered_ids.iter().enumerate() {
            rank.entry(*id).or_insert(idx);
        }
        self.queues
            .sort_by_key(|q| rank.get(&q.id).copied().unwrap_or(usize::MAX));
    }

    /// Appends `task_id` to `queue_id` if not already queued.
    pub fn enqueue(&mut self, queue_id: Uuid, task_id: Uuid) -> bool {
        let Some(q) = self.queues.iter_mut().find(|q| q.id == queue_id) else {
            return false;
        };
        if !q.task_ids.contains(&task_id) {
            q.task_ids.push(task_id);
        }
        true
    }

    /// Dequeues the next task from `queue_id` if enabled and below `max_concurrent`.
    pub fn dequeue(&mut self, queue_id: Uuid) -> Option<Uuid> {
        let active_len = self
            .active_tasks
            .get(&queue_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0);

        let q = self.queues.iter_mut().find(|q| q.id == queue_id)?;
        if !q.enabled || active_len >= q.max_concurrent || q.task_ids.is_empty() {
            return None;
        }

        let task_id = q.task_ids.remove(0);
        self.active_tasks.entry(queue_id).or_default().push(task_id);
        Some(task_id)
    }

    /// Releases an active concurrency slot for `task_id` in `queue_id`.
    pub fn complete_task(&mut self, queue_id: Uuid, task_id: Uuid) -> bool {
        let Some(active) = self.active_tasks.get_mut(&queue_id) else {
            return false;
        };
        let before = active.len();
        active.retain(|id| *id != task_id);
        active.len() != before
    }

    pub fn active_count(&self, queue_id: Uuid) -> u32 {
        self.active_tasks
            .get(&queue_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crud_and_reorder_queues() {
        let mut mgr = QueueManager::new();
        let q1 = mgr.create("Nightly", 2, Some(5_000_000));
        let q2 = mgr.create("Default", 4, None);

        assert!(mgr.rename(q1.id, "Overnight"));
        assert_eq!(mgr.get(q1.id).unwrap().name, "Overnight");

        mgr.reorder(&[q2.id, q1.id]);
        assert_eq!(mgr.queues()[0].id, q2.id);
        assert_eq!(mgr.queues()[1].id, q1.id);

        assert!(mgr.delete(q1.id));
        assert_eq!(mgr.queues().len(), 1);
    }

    #[test]
    fn dequeue_respects_max_concurrent_and_completion() {
        let mut mgr = QueueManager::new();
        let q = mgr.create("Limited", 2, None);
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        let t3 = Uuid::new_v4();

        assert!(mgr.enqueue(q.id, t1));
        assert!(mgr.enqueue(q.id, t2));
        assert!(mgr.enqueue(q.id, t3));

        assert_eq!(mgr.dequeue(q.id), Some(t1));
        assert_eq!(mgr.dequeue(q.id), Some(t2));
        assert_eq!(mgr.dequeue(q.id), None);
        assert_eq!(mgr.active_count(q.id), 2);

        assert!(mgr.complete_task(q.id, t1));
        assert_eq!(mgr.dequeue(q.id), Some(t3));
    }
}
