`cseq` is the Rust library (by Piotr Beling) of compact sequences.

# Example

```rust
use cseq::elias_fano;

let ef = elias_fano::Sequence::with_items_from_slice(&[0u64, 1, 801, 920, 999]);
assert_eq!(ef.get(2), Some(801));   // get value at index
assert_eq!(ef.get(5), None);
assert_eq!(ef.iter().collect::<Vec<_>>(), [0, 1, 801, 920, 999]);
assert_eq!(ef.iter().rev().collect::<Vec<_>>(), [999, 920, 801, 1, 0]);
assert_eq!(ef.geq_cursor(801).collect::<Vec<_>>(), [801, 920, 999]);
assert_eq!(ef.geq_cursor(802).collect::<Vec<_>>(), [920, 999]);
let mut c = ef.cursor_of(801).unwrap(); // find the item by value
assert_eq!(c.index(), 2);
assert_eq!(c.value(), Some(801));
c.advance();                    // and its successors:
assert_eq!(c.index(), 3);
assert_eq!(c.value(), Some(920));   
c.advance();
assert_eq!(c.index(), 4);
assert_eq!(c.value(), Some(999));
c.advance();
assert_eq!(c.index(), 5);
assert_eq!(c.value(), None);
```
