/*#[inline] // this is much slower than the ones below (which is adopted from standard library)
pub(super) fn partition_point<T, P>(tab: &[T], mut pred: P) -> usize
    where P: FnMut(&T, usize) -> bool
{
    let mut len = tab.len();
    let mut first = 0;
    while len > 0 {
        let half = len / 2;
        let mid = first + half; 
        if pred(unsafe { tab.get_unchecked(mid) }, mid) {   // tab[mid] < value
            first = mid + 1; 
            len = len - half - 1;
        } else {
            len = half;
        }
    }
    return first;
}*/

#[inline]
pub(super) fn partition_point_with_index<T, F>(tab: &[T], mut f: F) -> usize
where F: FnMut(&T, usize) -> bool,
{
    // INVARIANTS:
    // - 0 <= left <= left + size = right <= self.len()
    // - f returns true for everything in self[..left]
    // - f returns false for everything in self[right..]
    let mut size = tab.len();
    let mut left = 0;
    let mut right = size;
    while size > 0 {    // size > 0 | in original code was: left < right
        let mid = left + size / 2;
        if f(unsafe { tab.get_unchecked(mid) }, mid) {
            left = mid + 1;
        } else {
            right = mid;
        }
        size = right - left;
    }
    right   // left==right | in original code was: left
}

/*#[inline]
pub(super) fn partition_index<F>(mut size: usize, mut f: F) -> usize
where F: FnMut(usize) -> bool,
{
    // INVARIANTS:
    // - 0 <= left <= left + size = right <= self.len()
    // - f returns true for everything in self[..left]
    // - f returns false for everything in self[right..]
    let mut left = 0;
    let mut right = size;
    while size > 0 {    // size > 0 | in original code was: left < right
        let mid = left + size / 2;
        if f(mid) {
            left = mid + 1;
        } else {
            right = mid;
        }
        size = right - left;
    }
    right   // left==right | in original code was: left
}*/

/*#[inline]
pub(super) fn partition_index<F>(mut left: usize, mut right: usize, mut f: F) -> usize
where F: FnMut(usize) -> bool,
{
    // INVARIANTS:
    // - 0 <= left <= left + size = right <= self.len()
    // - f returns true for everything in self[..left]
    // - f returns false for everything in self[right..]
    let mut size = right - left;
    while size > 0 {    // size > 0 | in original code was: left < right
        let mid = left + size / 2;
        if f(mid) {
            left = mid + 1;
        } else {
            right = mid;
        }
        size = right - left;
    }
    right   // left==right | in original code was: left
}*/