use std::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DepotId<T> {
    marker: PhantomData<T>,
    index: u32,
    seq: u32,
}

struct Slot<T> {
    free: bool,
    seq: u32,
    value: T,
}

pub struct Depot<T> {
    slots: Vec<Slot<T>>,
    size: usize,
    seq: u32,
}

impl<T> Depot<T> {
    pub fn new() -> Depot<T> {
        Depot {
            slots: vec![],
            size: 0,
            seq: 0,
        }
    }

    pub fn push(&mut self, item: T) -> DepotId<T> {
        self.seq += 1;
        let idx: u32;
        let insert_pos = self.slots.iter().position(|s| s.free);
        if insert_pos.is_none() {
            let slot = Slot {
                seq: self.seq,
                free: true,
                value: item,
            };
            idx = self.slots.len() as u32;
            self.slots.push(slot);
        } else {
            idx = insert_pos.unwrap() as u32;
            self.slots[idx as usize].free = false;
            self.slots[idx as usize].free = false;
            self.slots[idx as usize].value = item;
        }
        DepotId {
            index: idx,
            seq: self.seq,
            marker: PhantomData,
        }
    }

    pub fn remove(&mut self, id: DepotId<T>) {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION");
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION");
        }
        self.slots[id.index as usize].free = true;
        self.size -= 1;
    }

    pub fn get(&self, id: DepotId<T>) -> &T {
        if id.index as usize >= self.slots.len() {
            panic!("GURU MEDITATION");
        }
        if self.slots[id.index as usize].seq != id.seq {
            panic!("GURU MEDITATION");
        }
        &self.slots[id.index as usize].value
    }

    pub fn ids(&self) -> DepotIterator<T> {
        DepotIterator {
            depot: self,
            index: 0,
            marker: PhantomData,
        }
    }
}

pub struct DepotIterator<'a, T> {
    depot: &'a Depot<T>,
    index: usize,
    marker: PhantomData<T>,
}

impl<'a, T> Iterator for DepotIterator<'a, T> {
    type Item = DepotId<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = None;
        while self.index < self.depot.slots.len() && ret.is_none() {
            if !self.depot.slots[self.index].free {
                ret = Some(DepotId {
                    index: self.index as u32,
                    seq: self.depot.slots[self.index].seq,
                    marker: PhantomData,
                });
            }
            self.index += 1;
        }
        ret
    }
}
