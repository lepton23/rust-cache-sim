
// use same data type for each field of struct for cache alignment
#[derive(Debug)]
pub struct CacheMeta {
        pub name:          String,
        pub level:          u8, // 0 = l1, 1 = l2, 2 = l3, single named caches = 0
        pub size:           u64,
        pub line_size:      u64,
        pub kind:           u8, // 0 = direct, 1 = 2way, 2 = 4way, 3 = 8way, 4 = fullassoc
        pub rp:             u8, // 0 = rr, 1 = lru, 2 = lfu
}