use std::marker::PhantomData;

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

    pub fn remove(&mut self, id: DepotId) -> T {
        
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq {} {}", self.slots[id.index as usize].seq , id.seq );
        }
        self.slots[id.index as usize].free = true;
        self.size -= 1;
        return std::mem::take(&mut self.slots[id.index as usize].value);
    }

    pub fn get(&self, id: DepotId) -> &T {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq {} {}", self.slots[id.index as usize].seq , id.seq );
        }
        &self.slots[id.index as usize].value
    }

    pub fn get_mut(&mut self, id: DepotId) -> &mut T {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION id.index > slots.len {} {}", id.index, self.slots.len());
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION invalid seq {} {}", self.slots[id.index as usize].seq , id.seq );
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

    pub fn iter(&self) -> DepotRefIterator<T> {
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

pub struct DepotIdIterator<'a, T>
where
    T: Default + HasId
{
    depot: &'a Depot<T>,
    index: usize,
    marker: PhantomData<T>,
}

impl<'a, T> Iterator for DepotIdIterator<'a, T>
where
    T: Default + HasId
{
    type Item = DepotId;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = None;
        while self.index < self.depot.slots.len() && ret.is_none() {
            if !self.depot.slots[self.index].free {
                ret = Some(DepotId {
                    index: self.index as u32,
                    seq: self.depot.slots[self.index].seq,
                });
            }
            self.index += 1;
        }
        ret
    }
}
