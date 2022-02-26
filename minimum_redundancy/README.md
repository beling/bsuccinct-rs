`minimum_redundancy` is the Rust library by Piotr Beling to encode and decode data
with binary or non-binary Huffman coding.

It can construct optimal prefix (minimum-redundancy) codes.
It can assign the codes whose bit-sizes are dividable by given length (1-bit, 2-bits, ...).
<!--- or even uses tree degree given. --->

The library uses modified Huffman algorithm, with ideas from papers:
- A. Brodnik, S. Carlsson, *Sub-linear decoding of Huffman Codes Almost In-Place*
- A. Moffat, J. Katajainen, *In-Place Calculation of Minimum-Redundancy Codes*

