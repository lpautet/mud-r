//use std::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DepotId {
    index: u32,
    seq: u32,
}

impl Default for DepotId {
    fn default() -> Self {
        Self { index: Default::default(), seq: Default::default() }
    }
}

pub trait HasId {
    fn id(&self) -> DepotId;
    fn set_id(&mut self, id: DepotId);
}

struct Slot<T> {
    free: bool,
    seq: u32,
    value: T,
}

pub struct Depot<T>
where
    T: Default + HasId
{
    slots: Vec<Slot<T>>,
    size: usize,
    seq: u32,
}

impl<T> Depot<T>
where
    T: Default + HasId
{
    pub fn new() -> Depot<T> {
        Depot {
            slots: vec![],
            size: 0,
            seq: 0,
        }
    }

    pub fn clear(&mut self) {
        self.size = 0;
        self.slots.clear()
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn push(&mut self, item: T) -> DepotId {
        if item.id() != DepotId::default() {
            panic!("GURU MEDITATION inserting with existing ID ! id.index={} id.seq={}", item.id().index, item.id().seq);
        }
        self.seq += 1;
        let idx: u32;
        let insert_pos = self.slots.iter().position(|s| s.free);
        if insert_pos.is_none() {
            let slot = Slot {
                seq: self.seq,
                free: false,
                value: item,
            };
            idx = self.slots.len() as u32;
            self.slots.push(slot);
        } else {
            idx = insert_pos.unwrap() as u32;
            self.slots[idx as usize].free = false;
            self.slots[idx as usize].seq = self.seq;
            self.slots[idx as usize].value = item;
        }
        self.size += 1;
        let ret = DepotId {
            index: idx,
            seq: self.seq,
        };
        self.get_mut(ret).set_id(ret);
        ret
    }

    pub fn take(&mut self, id: DepotId) -> T {
        
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq in take expected:{} got:{}-{}", self.slots[id.index as usize].seq , id.index, id.seq );
        }
        self.slots[id.index as usize].free = true;
        self.slots[id.index as usize].seq = 0; // so that old DepotId are now invalid
        self.size -= 1;
        return std::mem::take(&mut self.slots[id.index as usize].value);
    }

    pub fn get(&self, id: DepotId) -> &T {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq in get expected:{} got:{}-{}", self.slots[id.index as usize].seq ,id.index, id.seq );
        }
        &self.slots[id.index as usize].value
    }

    pub fn get_mut(&mut self, id: DepotId) -> &mut T {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq in get_mut expected:{} got:{}-{}", self.slots[id.index as usize].seq ,id.index, id.seq );
        }
        &mut self.slots[id.index as usize].value
    }

    pub fn ids(&self) -> Vec<DepotId> {
        let mut index = 0;
        let mut ret = vec![];
        while index < self.slots.len() {
            if !self.slots[index].free {
                ret.push(DepotId {
                    index: index as u32,
                    seq: self.slots[index].seq,
                });
            }
            index += 1;
        }
        ret
    }

    // pub fn iter_ids(&self) -> DepotIdIterator<T> {
    //     DepotIdIterator {
    //         depot: self,
    //         index: 0,
    //         marker: PhantomData,
    //     }
    // }

    pub fn iter(&self) -> DepotRefIterator<'_, T> {
        DepotRefIterator {
            depot: self,
            index: 0,
        }
    }

    // pub fn iter_mut(&mut self) -> DepotMutIterator<T> {
    //     DepotMutIterator {
    //         depot: self,
    //         index: 0,
    //         marker: PhantomData,
    //     }
    // }
}

pub struct DepotRefIterator<'a, T>
where
    T: Default + HasId
{
    depot: &'a Depot<T>,
    index: usize,
}

impl<'a, T> Iterator for DepotRefIterator<'a, T>
where
    T: Default + HasId
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = None;
        while self.index < self.depot.slots.len() && ret.is_none() {
            if !self.depot.slots[self.index].free {
                ret = Some(&self.depot.slots[self.index].value);
            }
            self.index += 1;
        }
        ret
    }
}

// pub struct DepotMutIterator<'a, T>
// where
//     T: Default,
// {
//     depot: &'a mut Depot<T>,
//     index: usize,
// }

// impl<'a, T> Iterator for DepotMutIterator<'a, T> {
//     type Item = &'a mut T;

//     fn next(&mut self) -> Option<&'a mut T> {
//         let mut ret = None;
//         while self.index < self.depot.slots.len() && ret.is_none() {
//             if !self.depot.slots[self.index].free {
//                 ret = Some(&mut self.depot.slots[self.index].value);
//             }
//             self.index += 1;
//         }
//         ret
//     }
// }

// pub struct DepotIdIterator<'a, T>
// where
//     T: Default + HasId
// {
//     depot: &'a Depot<T>,
//     index: usize,
//     marker: PhantomData<T>,
// }

// impl<'a, T> Iterator for DepotIdIterator<'a, T>
// where
//     T: Default + HasId
// {
//     type Item = DepotId;

//     fn next(&mut self) -> Option<Self::Item> {
//         let mut ret = None;
//         while self.index < self.depot.slots.len() && ret.is_none() {
//             if !self.depot.slots[self.index].free {
//                 ret = Some(DepotId {
//                     index: self.index as u32,
//                     seq: self.depot.slots[self.index].seq,
//                 });
//             }
//             self.index += 1;
//         }
//         ret
//     }
// }

#[cfg(test)]
mod depot_tests {
    use super::*;

    #[derive(Default, Debug, PartialEq)]
    struct TestItem {
        id: DepotId,
        value: i32,
    }

    impl HasId for TestItem {
        fn id(&self) -> DepotId {
            self.id
        }

        fn set_id(&mut self, id: DepotId) {
            self.id = id;
        }
    }

    #[test]
    fn test_depot_new() {
        let depot: Depot<TestItem> = Depot::new();
        assert!(depot.is_empty());
        assert_eq!(depot.len(), 0);
    }

    #[test]
    fn test_depot_push_and_get() {
        let mut depot: Depot<TestItem> = Depot::new();
        let item = TestItem { id: DepotId::default(), value: 42 };
        
        let id = depot.push(item);
        assert_eq!(depot.len(), 1);
        assert!(!depot.is_empty());
        
        let retrieved = depot.get(id);
        assert_eq!(retrieved.value, 42);
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn test_depot_multiple_items() {
        let mut depot: Depot<TestItem> = Depot::new();
        
        let id1 = depot.push(TestItem { id: DepotId::default(), value: 10 });
        let id2 = depot.push(TestItem { id: DepotId::default(), value: 20 });
        let id3 = depot.push(TestItem { id: DepotId::default(), value: 30 });
        
        assert_eq!(depot.len(), 3);
        assert_eq!(depot.get(id1).value, 10);
        assert_eq!(depot.get(id2).value, 20);
        assert_eq!(depot.get(id3).value, 30);
    }

    #[test]
    fn test_depot_take() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id = depot.push(TestItem { id: DepotId::default(), value: 100 });
        
        assert_eq!(depot.len(), 1);
        let taken = depot.take(id);
        assert_eq!(taken.value, 100);
        assert_eq!(depot.len(), 0);
        assert!(depot.is_empty());
    }

    #[test]
    fn test_depot_get_mut() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id = depot.push(TestItem { id: DepotId::default(), value: 50 });
        
        {
            let item_mut = depot.get_mut(id);
            item_mut.value = 75;
        }
        
        assert_eq!(depot.get(id).value, 75);
    }

    #[test]
    fn test_depot_ids() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id1 = depot.push(TestItem { id: DepotId::default(), value: 1 });
        let id2 = depot.push(TestItem { id: DepotId::default(), value: 2 });
        
        let ids = depot.ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_depot_iterator() {
        let mut depot: Depot<TestItem> = Depot::new();
        depot.push(TestItem { id: DepotId::default(), value: 10 });
        depot.push(TestItem { id: DepotId::default(), value: 20 });
        depot.push(TestItem { id: DepotId::default(), value: 30 });
        
        let values: Vec<i32> = depot.iter().map(|item| item.value).collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&10));
        assert!(values.contains(&20));
        assert!(values.contains(&30));
    }

    #[test]
    fn test_depot_reuse_slots() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id1 = depot.push(TestItem { id: DepotId::default(), value: 1 });
        let id2 = depot.push(TestItem { id: DepotId::default(), value: 2 });
        
        // Remove first item
        depot.take(id1);
        assert_eq!(depot.len(), 1);
        
        // Add new item - should reuse the slot
        let id3 = depot.push(TestItem { id: DepotId::default(), value: 3 });
        assert_eq!(depot.len(), 2);
        
        // Verify both items exist
        assert_eq!(depot.get(id2).value, 2);
        assert_eq!(depot.get(id3).value, 3);
    }

    #[test]
    fn test_depot_clear() {
        let mut depot: Depot<TestItem> = Depot::new();
        depot.push(TestItem { id: DepotId::default(), value: 1 });
        depot.push(TestItem { id: DepotId::default(), value: 2 });
        
        assert_eq!(depot.len(), 2);
        depot.clear();
        assert_eq!(depot.len(), 0);
        assert!(depot.is_empty());
    }

    #[test]
    fn test_depot_sequence_increment() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id1 = depot.push(TestItem { id: DepotId::default(), value: 1 });
        let id2 = depot.push(TestItem { id: DepotId::default(), value: 2 });
        
        // IDs should have different sequence numbers
        assert_ne!(id1, id2);
        
        // Remove and add again - should get different sequence
        depot.take(id1);
        let id3 = depot.push(TestItem { id: DepotId::default(), value: 3 });
        assert_ne!(id1, id3); // Different sequence even though same slot
    }

    #[test]
    #[should_panic(expected = "GURU MEDITATION invalid seq")]
    fn test_depot_panic_on_invalid_seq() {
        let mut depot: Depot<TestItem> = Depot::new();
        let id = depot.push(TestItem { id: DepotId::default(), value: 1 });
        depot.take(id);
        
        // Try to access with old ID - should panic
        depot.get(id);
    }
}

