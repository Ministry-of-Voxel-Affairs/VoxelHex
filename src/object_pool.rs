use std::{
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    vec::Vec,
};

pub fn empty_marker<T: num_traits::Bounded>() -> T {
    T::max_value()
}

#[cfg(feature = "bytecode")]
use bendy::{
    decoding::{FromBencode, Object},
    encoding::{Error as BencodeError, SingleItemEncoder, ToBencode},
};

//####################################################################################
//     ███████    ███████████        █████ ██████████   █████████  ███████████
//   ███░░░░░███ ░░███░░░░░███      ░░███ ░░███░░░░░█  ███░░░░░███░█░░░███░░░█
//  ███     ░░███ ░███    ░███       ░███  ░███  █ ░  ███     ░░░ ░   ░███  ░
// ░███      ░███ ░██████████        ░███  ░██████   ░███             ░███
// ░███      ░███ ░███░░░░░███       ░███  ░███░░█   ░███             ░███
// ░░███     ███  ░███    ░███ ███   ░███  ░███ ░   █░░███     ███    ░███
//  ░░░███████░   ███████████ ░░████████   ██████████ ░░█████████     █████
//    ░░░░░░░    ░░░░░░░░░░░   ░░░░░░░░   ░░░░░░░░░░   ░░░░░░░░░     ░░░░░
//  ███████████     ███████       ███████    █████
// ░░███░░░░░███  ███░░░░░███   ███░░░░░███ ░░███
//  ░███    ░███ ███     ░░███ ███     ░░███ ░███
//  ░██████████ ░███      ░███░███      ░███ ░███
//  ░███░░░░░░  ░███      ░███░███      ░███ ░███
//  ░███        ░░███     ███ ░░███     ███  ░███      █
//  █████        ░░░███████░   ░░░███████░   ███████████
//####################################################################################

/// Data required for the operation of ObjectPool
#[derive(Clone)]
struct ObjectPoolMetaData {
    /// Array of "reserved" state for stored elements
    items_reserved: Vec<bool>,

    /// Index of the first available item
    first_available: usize,

    /// The Capacity of the ObjectPool buffer vector
    /// aligns to Vec::capacity
    capacity: usize,
}

/// Stores re-usable objects to eliminate data allocation overhead when inserting and removing Nodes
/// It keeps track of different buffers for different levels in the graph, allocating more space initially to lower levels
#[derive(Clone)]
pub(crate) struct ObjectPool<T> {
    /// Pool of objects to be reused
    buffer: Vec<Arc<RwLock<T>>>,

    /// Statistics about the stored data
    meta: ObjectPoolMetaData,
}

#[cfg(feature = "bytecode")]
impl<T> ToBencode for ObjectPool<T>
where
    T: ToBencode + Default + Clone,
{
    const MAX_DEPTH: usize = 8;
    fn encode(&self, encoder: SingleItemEncoder) -> Result<(), BencodeError> {
        encoder.emit_list(|e| {
            e.emit(self.meta.capacity)?;
            for index in 0..self.buffer.len() {
                if self.meta.items_reserved[index] {
                    // Item is in use, write it out!
                    let item = self.buffer[index]
                        .read()
                        .expect("Expected to be able to read ObjectPool buffer");
                    e.emit(item.clone())?;
                }
            }
            // Emit list end token
            e.emit("#")?;
            Ok(())
        })
    }
}

#[cfg(feature = "bytecode")]
impl<T> FromBencode for ObjectPool<T>
where
    T: FromBencode + Default + Clone,
{
    fn decode_bencode_object(data: Object) -> Result<Self, bendy::decoding::Error> {
        match data {
            Object::List(mut list) => {
                let capacity = match list.next_object()?.unwrap() {
                    Object::Integer(i) => Ok(i.parse::<usize>().ok().unwrap()),
                    _ => Err(bendy::decoding::Error::unexpected_token(
                        "ObjectPool int field capacity",
                        "Something else",
                    )),
                }?;
                let mut items_reserved = Vec::with_capacity(capacity);
                let mut buffer = Vec::with_capacity(capacity);
                loop {
                    let next_object = list
                        .next_object()?
                        .expect("Expected another object within ObjectPool");
                    match next_object {
                        Object::Bytes(b) => {
                            // A token means the end of the object stream
                            debug_assert!(matches!(
                                String::from_utf8(b.to_vec())
                                    .unwrap_or("".to_string())
                                    .as_str(),
                                "#"
                            ));
                            break;
                        }
                        _ => {
                            buffer.push(Arc::new(RwLock::new(T::decode_bencode_object(
                                next_object,
                            )?)));
                            items_reserved.push(true);
                        }
                    }
                    if items_reserved.len() >= capacity {
                        debug_assert!(
                            false,
                            "More items in ObjectPool than the stored capacity of {capacity}"
                        );
                        break;
                    }
                }

                Ok(Self {
                    meta: ObjectPoolMetaData {
                        first_available: items_reserved.len(),
                        items_reserved,
                        capacity,
                    },
                    buffer,
                })
            }
            _ => Err(bendy::decoding::Error::unexpected_token(
                "List of ObjectPool<T> fields",
                "Something else",
            )),
        }
    }
}

#[allow(dead_code)]
impl<T> ObjectPool<T>
where
    T: Default + Clone,
{
    /// Create Objectpool with given capacity
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        ObjectPool {
            buffer: Vec::with_capacity(capacity),
            meta: ObjectPoolMetaData {
                capacity,
                items_reserved: Vec::with_capacity(capacity),
                first_available: 0,
            },
        }
    }

    /// pushes the next available marker into the next node, in case it's not reserved
    /// Returns true if first_available index points to available node
    fn try_set_next_available(&mut self) -> bool {
        if (self.meta.first_available + 1) < self.buffer.len()
            && !self.meta.items_reserved[self.meta.first_available]
        {
            return true;
        }

        if (self.meta.first_available + 1) < self.buffer.len()
            && !self.meta.items_reserved[self.meta.first_available + 1]
        {
            self.meta.first_available += 1;
            return true;
        }

        false
    }

    /// Length of the ObjectPool
    pub(crate) fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Pushes the given item into the pool
    pub(crate) fn push(&mut self, item: T) -> usize {
        let key = self.allocate();
        *self.get_mut(key) = item;
        key
    }

    /// Reserves an item within the pool and provides the key for it
    pub(crate) fn allocate(&mut self) -> usize {
        let key = if self.try_set_next_available() {
            let first_available = self.meta.first_available;
            self.meta.items_reserved[first_available] = true;
            self.try_set_next_available();
            first_available
        } else {
            debug_assert_eq!(self.meta.items_reserved.len(), self.buffer.len());

            // reserve place for additional items, but
            // reserve less additional items for larger buffers
            let x = self.buffer.len().max(10) as f32;
            let new_item_count = ((100. * x.log10().powf(2.)) / x) as usize;

            self.buffer.reserve(new_item_count);
            self.meta.items_reserved.reserve(new_item_count);

            // mark item as reserved and return with the key
            self.meta.items_reserved.push(true);
            self.buffer.push(Arc::default());
            self.buffer.len() - 1
        };
        self.try_set_next_available();
        key
    }

    /// Returns the ownership of the item under the given key from the pool
    pub(crate) fn pop(&mut self, key: usize) -> T {
        debug_assert!(self.key_is_valid(key));
        let mut item = self.buffer[key]
            .write()
            .expect("Expected to be able to update ReusableItem in Object pool");

        self.meta.items_reserved[key] = false;
        self.meta.first_available = self.meta.first_available.min(key);
        std::mem::take(&mut item)
    }

    pub(crate) fn free(&mut self, key: usize) -> bool {
        if self.key_is_valid(key) {
            self.meta.items_reserved[key] = false;
            self.meta.first_available = self.meta.first_available.min(key);
            true
        } else {
            false
        }
    }

    pub(crate) fn get(&'_ self, key: usize) -> RwLockReadGuard<'_, T> {
        debug_assert!(self.key_is_valid(key));
        self.buffer[key]
            .read()
            .expect("Expected to be able to read ReusableItem in Object pool")
    }

    pub(crate) fn get_mut(&'_ mut self, key: usize) -> RwLockWriteGuard<'_, T> {
        debug_assert!(self.key_is_valid(key));
        self.buffer[key]
            .write()
            .expect("Expected to be able to update ReusableItem in Object pool")
    }

    pub(crate) fn swap(&mut self, src: usize, dst: usize) {
        self.buffer.swap(src, dst);
    }

    pub(crate) fn key_is_valid(&self, key: usize) -> bool {
        key < self.buffer.len() && self.meta.items_reserved[key]
    }
}

#[cfg(test)]
mod object_pool_tests {
    use crate::object_pool::ObjectPool;
    use rayon::prelude::*;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_push_pop_modify() {
        let mut pool = ObjectPool::<f32>::with_capacity(3);
        let test_value = 5.;
        let key = pool.push(test_value);
        assert_eq!(*pool.get(key), test_value);

        *pool.get_mut(key) = 10.;
        assert_eq!(*pool.get(key), 10.);
        assert_eq!(pool.pop(key), 10.);
        assert!(!pool.free(key));
    }

    #[test]
    fn test_push_pop_modify_multiple_threads() {
        let mut pool = ObjectPool::<f32>::with_capacity(3);
        let test_value = 5.;
        let key = pool.push(test_value);
        assert_eq!(*pool.get(key), test_value);

        *pool.get_mut(key) = 10.;
        assert_eq!(*pool.get(key), 10.);
        assert_eq!(pool.pop(key), 10.);
        assert!(!pool.free(key));
    }

    #[test]
    fn test_push_deallocate() {
        let mut pool = ObjectPool::<f32>::with_capacity(3);
        let test_value = 5.;
        let key = pool.push(test_value);
        assert!(*pool.get(key) == test_value);

        pool.free(key);
        assert!(!pool.free(key));
    }

    #[test]
    fn test_edge_case_reused_item() {
        let mut pool = ObjectPool::<f32>::with_capacity(3);
        let test_value = 5.;
        let key_1 = pool.push(test_value);
        pool.push(test_value * 2.);
        pool.pop(key_1);
        assert_eq!(pool.meta.first_available, 0); // the first item should be available

        pool.push(test_value * 3.);
        assert!(*pool.get(key_1) == test_value * 3.); // the original key is reused to hold the latest value
    }

    #[test]
    fn test_singlethreaded_insert() {
        let item_count = 10;
        let mut object_pool = ObjectPool::<u32>::with_capacity(item_count / 2);

        let items = (0..item_count)
            .map(|i| (object_pool.push(i as u32), i as u32))
            .collect::<Vec<_>>();

        for (key, value) in items {
            assert_eq!(*object_pool.get(key), value);
        }
    }

    #[test]
    fn test_multithreaded_insert_and_update() {
        let item_count = 10;
        let object_pool = Arc::new(RwLock::new(ObjectPool::<u32>::with_capacity(
            item_count / 2,
        )));

        let items = (0..item_count)
            .into_par_iter()
            .map(|i| (object_pool.write().unwrap().push(i as u32), i as u32))
            .collect::<Vec<_>>();

        for (key, value) in &items {
            assert_eq!(*object_pool.read().unwrap().get(*key), *value);
        }

        // Update each element in multiple threads
        (0..item_count * 2).into_par_iter().for_each(|i| {
            let mut pool = object_pool.write().unwrap();
            let key = items[i % item_count].0;
            *pool.get_mut(key) *= 2;
        });
        (0..item_count * 2).into_par_iter().for_each(|i| {
            let mut pool = object_pool.write().unwrap();
            let key = items[i % item_count].0;
            *pool.get_mut(key) *= 2;
        });

        for (key, value) in &items {
            assert_eq!(*object_pool.read().unwrap().get(*key), *value * 16);
        }
    }
}
