//! A THROWAWAY probe: what is the floor for an index-linked list in safe
//! Rust, doing exactly the work the other two arms do?
//!
//! This is not a proposal and it is not in the crate. It exists to size the
//! prize before any refactor, which is the instruction-counting skill's
//! rule: make the cheapest possible version, even an incomplete one, purely
//! to find the ceiling. If this lands above 20 instr/op the whole direction
//! is dead and no amount of polishing `list.rs` will save it.
//!
//! It prints the same checksum as `main.rs` and the C arm. Two instruction
//! counts of two different programs would not be a comparison.
//!
//! # The two things it does differently from `rusty_rtos_core::list`
//!
//! **One array, not two.** `list.rs` keeps `items: [Node; N]` and
//! `ends: [End; L]`, and encodes a link as "below `END_BASE` means an item,
//! at or above it means a marker". Every traversal therefore tests which
//! kind of link it holds before it can follow it. Here the end markers are
//! NODES, at the top of the same array -- which is exactly what C FreeRTOS
//! does: `xListEnd` is a `ListItem_t` living inside `List_t`, and that is
//! why its walk needs no test.
//!
//! **The array is a power of two and every index is masked.** `nodes` is 16
//! long and every access is `nodes[(link & 15) as usize]`. LLVM can prove
//! `x & 15 < 16`, so the bounds check folds away -- with no `unsafe`, no
//! `get_unchecked`, and a panic that is unreachable rather than suppressed.
//! The census says bounds checks are 19.6% of this arm.
//!
//! What it does NOT do, and what a real version would have to: reject an
//! invalid caller handle, report `Busy`/`NotActive`, support any `N` rather
//! than a power of two, or carry the round-robin's `Option`. Those cost
//! something, so the real figure will be above this one. That is the point
//! of a floor.

/// 8 items + 2 end markers, rounded up to a power of two so the mask works.
const SLOTS: usize = 16;
const MASK: u16 = 15;
const ITEMS: u16 = 8;
/// The end markers live at the top of the same array.
const READY_END: u16 = 8;
const DELAYED_END: u16 = 9;
const LISTS: usize = 2;
const READY: usize = 0;
const DELAYED: usize = 1;

#[derive(Clone, Copy)]
struct Node {
    value: u64,
    prev: u16,
    next: u16,
    container: u8,
}

const NO_LIST: u8 = u8::MAX;

/// What `List_t` keeps beside its `xListEnd`: the round-robin cursor and the
/// count. Touched once per operation, never per link followed.
#[derive(Clone, Copy)]
struct Meta {
    cursor: u16,
    len: u16,
    end: u16,
}

struct Lists {
    nodes: [Node; SLOTS],
    meta: [Meta; LISTS],
}

impl Lists {
    fn new() -> Self {
        let mut nodes = [Node {
            value: 0,
            prev: 0,
            next: 0,
            container: NO_LIST,
        }; SLOTS];
        // A marker points at itself and carries the largest value, so the
        // ordered walk stops on it without anyone testing for it. This is
        // `vListInitialise`: `xListEnd.xItemValue = portMAX_DELAY`.
        for end in [READY_END, DELAYED_END] {
            let at = (end & MASK) as usize;
            nodes[at].value = u64::MAX;
            nodes[at].next = end;
            nodes[at].prev = end;
        }
        Self {
            nodes,
            meta: [
                Meta {
                    cursor: READY_END,
                    len: 0,
                    end: READY_END,
                },
                Meta {
                    cursor: DELAYED_END,
                    len: 0,
                    end: DELAYED_END,
                },
            ],
        }
    }

    #[inline]
    fn at(&self, link: u16) -> &Node {
        &self.nodes[(link & MASK) as usize]
    }

    #[inline]
    fn at_mut(&mut self, link: u16) -> &mut Node {
        &mut self.nodes[(link & MASK) as usize]
    }

    /// `vListInsertEnd`: in front of the cursor.
    fn insert_end(&mut self, list: usize, item: u16) {
        let cursor = self.meta[list].cursor;
        let before = self.at(cursor).prev;
        let n = self.at_mut(item);
        n.next = cursor;
        n.prev = before;
        n.container = list as u8;
        self.at_mut(before).next = item;
        self.at_mut(cursor).prev = item;
        self.meta[list].len += 1;
    }

    /// `vListInsert`: ordered by value, after every item that compares equal.
    fn insert(&mut self, list: usize, item: u16, value: u64) {
        let end = self.meta[list].end;
        let mut iter = end;
        // The marker carries u64::MAX, so this stops there with no test.
        while self.at(self.at(iter).next).value <= value {
            iter = self.at(iter).next;
        }
        let after = self.at(iter).next;
        let n = self.at_mut(item);
        n.value = value;
        n.next = after;
        n.prev = iter;
        n.container = list as u8;
        self.at_mut(after).prev = item;
        self.at_mut(iter).next = item;
        self.meta[list].len += 1;
    }

    /// `listGET_OWNER_OF_NEXT_ENTRY`: advance, skipping the marker once.
    fn next_round_robin(&mut self, list: usize) -> u16 {
        let end = self.meta[list].end;
        let mut cursor = self.at(self.meta[list].cursor).next;
        if cursor == end {
            cursor = self.at(cursor).next;
        }
        self.meta[list].cursor = cursor;
        cursor
    }

    /// `uxListRemove`: returns the remaining count.
    fn remove(&mut self, item: u16) -> usize {
        let n = *self.at(item);
        let list = n.container as usize;
        self.at_mut(n.next).prev = n.prev;
        self.at_mut(n.prev).next = n.next;
        let m = &mut self.meta[list];
        if m.cursor == item {
            m.cursor = n.prev;
        }
        m.len -= 1;
        let left = m.len;
        self.at_mut(item).container = NO_LIST;
        usize::from(left)
    }
}

fn next(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

fn mix(sum: u64, value: u64) -> u64 {
    (sum ^ value).wrapping_mul(0x100_0000_01b3)
}

fn main() {
    let rounds: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(100_000);

    let mut lists = Lists::new();
    let mut sum: u64 = 0xcbf2_9ce4_8422_2325;
    let mut state: u32 = 0x1234_5678;

    for _ in 0..rounds {
        for item in 0..ITEMS {
            lists.insert_end(READY, item);
        }
        for _ in 0..ITEMS {
            sum = mix(sum, u64::from(lists.next_round_robin(READY)));
        }
        for item in 0..ITEMS {
            sum = mix(sum, lists.remove(item) as u64);
        }
        for item in 0..ITEMS {
            let value = u64::from(next(&mut state));
            lists.insert(DELAYED, item, value);
        }
        for item in 0..ITEMS {
            sum = mix(sum, lists.remove(item) as u64);
        }
    }

    println!("rounds={rounds} items={ITEMS} checksum={sum:016x}");
}
