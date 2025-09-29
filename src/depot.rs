use std::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DepotId {
    index: u32,
    seq: u32,
}

// Type-safe wrapper types for different entity kinds
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CharId(pub DepotId);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ObjId(pub DepotId);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TextId(pub DepotId);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DescriptorId(pub DepotId);

// Conversion traits for type-safe IDs
impl From<DepotId> for CharId {
    fn from(id: DepotId) -> Self {
        CharId(id)
    }
}

impl From<CharId> for DepotId {
    fn from(id: CharId) -> Self {
        id.0
    }
}

impl From<DepotId> for ObjId {
    fn from(id: DepotId) -> Self {
        ObjId(id)
    }
}

impl From<ObjId> for DepotId {
    fn from(id: ObjId) -> Self {
        id.0
    }
}

impl From<DepotId> for DescriptorId {
    fn from(id: DepotId) -> Self {
        DescriptorId(id)
    }
}

impl From<DescriptorId> for DepotId {
    fn from(id: DescriptorId) -> Self {
        id.0
    }
}

impl From<DepotId> for TextId {
    fn from(id: DepotId) -> Self {
        TextId(id)
    }
}

impl From<TextId> for DepotId {
    fn from(id: TextId) -> Self {
        id.0
    }
}

// Display implementations for type-safe IDs
impl std::fmt::Display for CharId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "char:{}", self.0)
    }
}

impl std::fmt::Display for ObjId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "obj:{}", self.0)
    }
}

impl std::fmt::Display for TextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text:{}", self.0)
    }
}

impl std::fmt::Display for DescriptorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "desc:{}", self.0)
    }
}

impl std::fmt::Display for DepotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.index, self.seq)
    }
}

// Type aliases for backwards compatibility and convenience
pub type CharDepot = Depot<crate::structs::CharData, CharId>;
pub type ObjDepot = Depot<crate::structs::ObjData, ObjId>;
pub type TextDepot = Depot<crate::TextData, TextId>;

pub trait HasId<ID = DepotId> {
    fn id(&self) -> ID;
    fn set_id(&mut self, id: ID);
}

struct Slot<T> {
    free: bool,
    seq: u32,
    value: T,
}

pub struct Depot<T, ID = DepotId>
where
    T: Default + HasId<ID>,
    ID: Copy + From<DepotId> + Into<DepotId> + PartialEq,
{
    slots: Vec<Slot<T>>,
    size: usize,
    seq: u32,
    _phantom: PhantomData<ID>,
}

impl<T, ID> Depot<T, ID>
where
    T: Default + HasId<ID>,
    ID: Copy + From<DepotId> + Into<DepotId> + PartialEq,
{
    pub fn default() -> Depot<T, ID> {
        Depot {
            slots: vec![],
            size: 0,
            seq: 0,
            _phantom: PhantomData,
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

    pub fn push(&mut self, item: T) -> ID {
        if item.id() != ID::from(DepotId::default()) {
            let depot_id: DepotId = item.id().into();
            panic!(
                "GURU MEDITATION inserting with existing ID ! id.index={} id.seq={}",
                depot_id.index, depot_id.seq
            );
        }
        self.seq += 1;
        let idx: u32;
        let insert_pos = self.slots.iter().position(|s| s.free);
        if let Some(insert_pos) = insert_pos {
            idx = insert_pos as u32;
            self.slots[idx as usize].free = false;
            self.slots[idx as usize].seq = self.seq;
            self.slots[idx as usize].value = item;
        } else {
            let slot = Slot {
                seq: self.seq,
                free: false,
                value: item,
            };
            idx = self.slots.len() as u32;
            self.slots.push(slot);
        }
        self.size += 1;
        let ret = DepotId {
            index: idx,
            seq: self.seq,
        };
        let typed_ret = ID::from(ret);
        self.get_mut(typed_ret).set_id(typed_ret);
        typed_ret
    }

    pub fn take(&mut self, id: ID) -> T {
        let depot_id: DepotId = id.into();
        if depot_id.index as usize >= self.slots.len() {
            panic!(
                "GURU MEDITATION id.index > slots.len {} {}",
                depot_id.index,
                self.slots.len()
            );
        }
        if self.slots[depot_id.index as usize].seq != depot_id.seq {
            panic!(
                "GURU MEDITATION invalid seq in take expected:{} got:{}-{}",
                self.slots[depot_id.index as usize].seq, depot_id.index, depot_id.seq
            );
        }
        self.slots[depot_id.index as usize].free = true;
        self.slots[depot_id.index as usize].seq = 0; // so that old DepotId are now invalid
        self.size -= 1;
        std::mem::take(&mut self.slots[depot_id.index as usize].value)
    }

    pub fn get(&self, id: ID) -> &T {
        let depot_id: DepotId = id.into();
        if depot_id.index as usize >= self.slots.len() {
            panic!(
                "GURU MEDITATION id.index > slots.len {} {}",
                depot_id.index,
                self.slots.len()
            );
        }
        if self.slots[depot_id.index as usize].seq != depot_id.seq {
            panic!(
                "GURU MEDITATION invalid seq in get expected:{} got:{}-{}",
                self.slots[depot_id.index as usize].seq, depot_id.index, depot_id.seq
            );
        }
        &self.slots[depot_id.index as usize].value
    }

    pub fn get_mut(&mut self, id: ID) -> &mut T {
        let depot_id: DepotId = id.into();
        if depot_id.index as usize >= self.slots.len() {
            panic!(
                "GURU MEDITATION id.index > slots.len {} {}",
                depot_id.index,
                self.slots.len()
            );
        }
        if self.slots[depot_id.index as usize].seq != depot_id.seq {
            panic!(
                "GURU MEDITATION invalid seq in get_mut expected:{} got:{}-{}",
                self.slots[depot_id.index as usize].seq, depot_id.index, depot_id.seq
            );
        }
        &mut self.slots[depot_id.index as usize].value
    }

    pub fn ids(&self) -> Vec<ID> {
        let mut index = 0;
        let mut ret = vec![];
        while index < self.slots.len() {
            if !self.slots[index].free {
                let depot_id = DepotId {
                    index: index as u32,
                    seq: self.slots[index].seq,
                };
                ret.push(ID::from(depot_id));
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

    pub fn iter(&self) -> DepotRefIterator<'_, T, ID> {
        DepotRefIterator {
            depot: self,
            index: 0,
            _phantom: PhantomData,
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

pub struct DepotRefIterator<'a, T, ID>
where
    T: Default + HasId<ID>,
    ID: Copy + From<DepotId> + Into<DepotId> + PartialEq,
{
    depot: &'a Depot<T, ID>,
    index: usize,
    _phantom: PhantomData<ID>,
}

impl<'a, T, ID> Iterator for DepotRefIterator<'a, T, ID>
where
    T: Default + HasId<ID>,
    ID: Copy + From<DepotId> + Into<DepotId> + PartialEq,
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

    impl HasId<DepotId> for TestItem {
        fn id(&self) -> DepotId {
            self.id
        }

        fn set_id(&mut self, id: DepotId) {
            self.id = id;
        }
    }

    #[test]
    fn test_depot_new() {
        let depot: Depot<TestItem> = Depot::default();
        assert!(depot.is_empty());
        assert_eq!(depot.len(), 0);
    }

    #[test]
    fn test_depot_push_and_get() {
        let mut depot: Depot<TestItem> = Depot::default();
        let item = TestItem {
            id: DepotId::default(),
            value: 42,
        };

        let id = depot.push(item);
        assert_eq!(depot.len(), 1);
        assert!(!depot.is_empty());

        let retrieved = depot.get(id);
        assert_eq!(retrieved.value, 42);
        assert_eq!(retrieved.id, id);
    }

    #[test]
    fn test_depot_multiple_items() {
        let mut depot: Depot<TestItem> = Depot::default();

        let id1 = depot.push(TestItem {
            id: DepotId::default(),
            value: 10,
        });
        let id2 = depot.push(TestItem {
            id: DepotId::default(),
            value: 20,
        });
        let id3 = depot.push(TestItem {
            id: DepotId::default(),
            value: 30,
        });

        assert_eq!(depot.len(), 3);
        assert_eq!(depot.get(id1).value, 10);
        assert_eq!(depot.get(id2).value, 20);
        assert_eq!(depot.get(id3).value, 30);
    }

    #[test]
    fn test_depot_take() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id = depot.push(TestItem {
            id: DepotId::default(),
            value: 100,
        });

        assert_eq!(depot.len(), 1);
        let taken = depot.take(id);
        assert_eq!(taken.value, 100);
        assert_eq!(depot.len(), 0);
        assert!(depot.is_empty());
    }

    #[test]
    fn test_depot_get_mut() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id = depot.push(TestItem {
            id: DepotId::default(),
            value: 50,
        });

        {
            let item_mut = depot.get_mut(id);
            item_mut.value = 75;
        }

        assert_eq!(depot.get(id).value, 75);
    }

    #[test]
    fn test_depot_ids() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id1 = depot.push(TestItem {
            id: DepotId::default(),
            value: 1,
        });
        let id2 = depot.push(TestItem {
            id: DepotId::default(),
            value: 2,
        });

        let ids = depot.ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_depot_iterator() {
        let mut depot: Depot<TestItem> = Depot::default();
        depot.push(TestItem {
            id: DepotId::default(),
            value: 10,
        });
        depot.push(TestItem {
            id: DepotId::default(),
            value: 20,
        });
        depot.push(TestItem {
            id: DepotId::default(),
            value: 30,
        });

        let values: Vec<i32> = depot.iter().map(|item| item.value).collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&10));
        assert!(values.contains(&20));
        assert!(values.contains(&30));
    }

    #[test]
    fn test_depot_reuse_slots() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id1 = depot.push(TestItem {
            id: DepotId::default(),
            value: 1,
        });
        let id2 = depot.push(TestItem {
            id: DepotId::default(),
            value: 2,
        });

        // Remove first item
        depot.take(id1);
        assert_eq!(depot.len(), 1);

        // Add new item - should reuse the slot
        let id3 = depot.push(TestItem {
            id: DepotId::default(),
            value: 3,
        });
        assert_eq!(depot.len(), 2);

        // Verify both items exist
        assert_eq!(depot.get(id2).value, 2);
        assert_eq!(depot.get(id3).value, 3);
    }

    #[test]
    fn test_depot_clear() {
        let mut depot: Depot<TestItem> = Depot::default();
        depot.push(TestItem {
            id: DepotId::default(),
            value: 1,
        });
        depot.push(TestItem {
            id: DepotId::default(),
            value: 2,
        });

        assert_eq!(depot.len(), 2);
        depot.clear();
        assert_eq!(depot.len(), 0);
        assert!(depot.is_empty());
    }

    #[test]
    fn test_depot_sequence_increment() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id1 = depot.push(TestItem {
            id: DepotId::default(),
            value: 1,
        });
        let id2 = depot.push(TestItem {
            id: DepotId::default(),
            value: 2,
        });

        // IDs should have different sequence numbers
        assert_ne!(id1, id2);

        // Remove and add again - should get different sequence
        depot.take(id1);
        let id3 = depot.push(TestItem {
            id: DepotId::default(),
            value: 3,
        });
        assert_ne!(id1, id3); // Different sequence even though same slot
    }

    #[test]
    #[should_panic(expected = "GURU MEDITATION invalid seq")]
    fn test_depot_panic_on_invalid_seq() {
        let mut depot: Depot<TestItem> = Depot::default();
        let id = depot.push(TestItem {
            id: DepotId::default(),
            value: 1,
        });
        depot.take(id);

        // Try to access with old ID - should panic
        depot.get(id);
    }
}
