`minimum_redundancy` is the Rust library by Piotr Beling to encode and decode data with binary or non-binary minimum-redundancy (Huffman) coding.

The library is fast and consumes low memory both to construct (which is done without explicitly building a tree) and store the coding dictionary (it only stores frequency-sorted symbols and the numbers of non-leaf nodes at successive levels of the canonical Huffman tree). In addition, the implementation is generic. It can construct not only binary codes (obtained via binary Huffman trees), but of any arity (trees of any degree). 

The high efficiency of `minimum_redundancy` is confirmed by benchmarks included in (please cite this paper if you are using `minimum_redundancy` for research purposes):
- Piotr Beling, *BSuccinct: Rust libraries and programs focused on succinct data structures*, SoftwareX, Volume 26, 2024, 101681, ISSN 2352-7110,
<https://doi.org/10.1016/j.softx.2024.101681>.

The library uses improved Huffman algorithm, with ideas from the following papers:
- A. Brodnik, S. Carlsson, *Sub-linear decoding of Huffman Codes Almost In-Place*, 1998
- A. Moffat, J. Katajainen, *In-Place Calculation of Minimum-Redundancy Codes*.
  In: Akl S.G., Dehne F., Sack JR., Santoro N. (eds) Algorithms and Data Structures.
  WADS 1995. Lecture Notes in Computer Science, vol 955. Springer, Berlin, Heidelberg.
  <https://doi.org/10.1007/3-540-60220-8_79>

# Example
```rust
use minimum_redundancy::{Coding, Code, DecodingResult, BitsPerFragment};
use maplit::hashmap;

// Construct coding with 1 bit per fragment for values 'a', 'b', 'c',
// whose frequencies of occurrence are 100, 50, 10 times, respectively.
let huffman = Coding::from_frequencies(BitsPerFragment(1), hashmap!('a' => 100u32, 'b' => 50, 'c' => 10));
// We expected the following Huffman tree:
//  /  \
// /\  a
// bc
// and the following code assignment: a -> 1, b -> 00, c -> 01
assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{ content: 0b1, len: 1 },
                'b' => Code{ content: 0b00, len: 2 },
                'c' => Code{ content: 0b01, len: 2 }
               ));
// reverse codes encode the first levels of the tree on the least significant bits (e.g., c -> 10):
assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{ content: 0b1, len: 1 },
                'b' => Code{ content: 0b00, len: 2 },
                'c' => Code{ content: 0b10, len: 2 }
               ));
let mut decoder_for_a = huffman.decoder();
assert_eq!(decoder_for_a.consume(1), DecodingResult::Value(&'a'));
let mut decoder_for_b = huffman.decoder();
assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
assert_eq!(decoder_for_b.consume(0), DecodingResult::Value(&'b'));
let mut decoder_for_c = huffman.decoder();
assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
assert_eq!(decoder_for_c.consume(1), DecodingResult::Value(&'c'));
assert_eq!(huffman.total_fragments_count(), 5); // 1+2+2
assert_eq!(huffman.values.as_ref(), ['a', 'b', 'c']); // sorted by frequencies
```